use super::fixture_roster;
use crate::{
    Command, compile_rooted_canary_for_native_host, fs, pass_canary, unique_no_output_build_dir,
};

#[test]
fn runtime_string_concat_membership_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_CONCAT_MEMBERSHIP_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-concat-membership-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string concat membership canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("string concat membership canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string concat membership canary should run");

    assert_eq!(
        output.status.code(),
        Some(71),
        "expected runtime string concat membership canary to preserve concat result and exit 71, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_string_field_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_STRING_FIELD_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime string field concat canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("string field concat canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(73),
        "expected runtime string field concat canary to preserve nested string writes and exit 73, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_string_field_concat_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_STRING_FIELD_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed string field concat canary should compile from its authored root",
    );
    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed string field concat canary should retain its executable receipt",
    );

    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(81),
        "expected runtime machine-owned indexed string field concat canary to preserve direct machine-owned indexed string writes and exit 81, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_indexed_bounded_carrier_literal_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-indexed-bounded-carrier-literal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned indexed bounded-carrier literal canary should compile from its authored root",
    );
    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned indexed bounded-carrier literal canary should retain its executable receipt",
    );

    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned indexed bounded-carrier literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(85),
        "expected indexed owned-carrier literal assignment and append to preserve inline bytes and exit 85, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_double_indexed_bounded_carrier_literal_exit_canary_runs() {
    let canary = pass_canary(
        fixture_roster::RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_BOUNDED_CARRIER_LITERAL_EXIT,
    );
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-double-indexed-bounded-carrier-literal-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned double-indexed bounded-carrier literal canary should compile from its authored root",
    );
    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned double-indexed bounded-carrier literal canary should retain its executable receipt",
    );

    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned double-indexed bounded-carrier literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(87),
        "expected double-indexed owned-carrier literal assignment and append to preserve inline bytes and exit 87, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_machine_owned_double_indexed_string_field_concat_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MACHINE_OWNED_DOUBLE_INDEXED_STRING_FIELD_CONCAT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-machine-owned-double-indexed-string-field-concat-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "runtime machine-owned double-indexed string field concat canary should compile from its authored root",
    );
    let executable = compilation.checked_native_executable_path().expect(
        "runtime machine-owned double-indexed string field concat canary should retain its executable receipt",
    );

    let output = Command::new(executable)
        .output()
        .expect("runtime machine-owned double-indexed string field concat canary should run");

    assert_eq!(
        output.status.code(),
        Some(83),
        "expected runtime machine-owned double-indexed string field concat canary to preserve double-runtime-indexed string writes and exit 83, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_machine_owned_parameter_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_MACHINE_OWNED_PARAMETER_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-machine-owned-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable machine-owned parameter write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable machine-owned parameter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable machine-owned parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(141),
        "expected runtime mutable machine-owned parameter write canary to preserve writes through mutable machine-owned call parameters and exit 141, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn mutable_local_parameter_write_loop_reaches_native_artifact() {
    // This original compile regression loops forever. Its separate exit
    // companion owns runtime observation; this owner must never execute it.
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_LOCAL_PARAMETER_WRITE_COMPILE);
    let scratch = unique_no_output_build_dir();
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mutable local parameter write loop should reach native code");
    compilation
        .retained_native_artifact()
        .expect("mutable parameter write loop should retain its native artifact")
        .validate()
        .expect("mutable parameter write loop artifact should replay");
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mutable_local_parameter_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_LOCAL_PARAMETER_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-local-parameter-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable local parameter write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable local parameter canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable local parameter write canary should run");

    assert_eq!(
        output.status.code(),
        Some(171),
        "expected runtime mutable local parameter write canary to preserve writes through local mutable call parameters and exit 171, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_mutable_parameter_read_modify_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_PARAMETER_READ_MODIFY_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-parameter-read-modify-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable parameter read/modify/write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mutable parameter RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable parameter read/modify/write canary should run");

    assert_eq!(
        output.status.code(),
        Some(191),
        "expected runtime mutable parameter read/modify/write canary to preserve aliased binary writes and exit 191, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

use super::fixture_roster;
use crate::{
    Command, check_canary, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, fail_canary, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn case_membership_value_exit_canary_runs() {
    // Decision 11: `cmd in Command::Move` in VALUE position lowers to a
    // tag-only compare. The constructed payload (`dx: 3`) exits 71 if the
    // compare reads payload bytes instead of clamping to the tag.
    let canary = pass_canary(fixture_roster::CASE_MEMBERSHIP_VALUE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-membership-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case membership value canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case membership value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case membership value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cmd in Command::Move` to be a true tag test (exit 70), got {:?} (71 = membership missed, e.g. payload bytes leaked into the compare)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn match_exhaustive_by_cases_canary_runs() {
    // Exhaustiveness over implicit case-domains: one arm per case counts as
    // a complete tag set (no `_`), and the counted dispatch still selects
    // the right arm at runtime.
    let canary = pass_canary(fixture_roster::MATCH_EXHAUSTIVE_BY_CASES);
    let scratch = std::env::temp_dir().join(format!(
        "omega-match-exhaustive-by-cases-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("exhaustive-by-cases canary should compile without a `_` arm");

    let executable = compilation
        .checked_native_executable_path()
        .expect("exhaustive-by-cases canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("exhaustive-by-cases canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the held case's arm (exit 70), got {:?} (71 = dispatch missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn match_exhaustive_by_case_union_domain_canary_runs() {
    // A PURE case-union domain arm (the sole body fact is
    // `self in Command::Move | Command::Say`) contributes its tag set to exhaustiveness
    // -- no `_` needed -- and classifies at runtime: the held value is the
    // SECOND union member, so a lowering that drops union arms exits 71.
    let canary = pass_canary(fixture_roster::MATCH_EXHAUSTIVE_BY_CASE_UNION_DOMAIN);
    let scratch = std::env::temp_dir().join(format!(
        "omega-match-exhaustive-union-domain-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case-union-domain canary should compile without a `_` arm");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case-union-domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case-union-domain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the union-domain arm to classify `Command::Say` (exit 70), got {:?} (71 = union membership missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_membership_union_guard_exit_canary_runs() {
    // Decision 11: a union of implicit case domains as a transition guard
    // subject; the held value matches the SECOND (payload-bearing) arm.
    let canary = pass_canary(fixture_roster::CASE_MEMBERSHIP_UNION_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-case-membership-union-guard-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("case membership union guard canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case membership union guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case membership union guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cmd in Command::Quit | Command::Move` to take the matched arm (exit 70), got {:?} (71 = union membership missed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_reassignment_exit_canary_runs() {
    // Overwriting one payload-carrying case with another (`Walk { pace: 9 }`
    // then `Run { speed: 70 }`) must rewrite both the tag and the overlaying
    // payload bytes: a stale tag selects the first (Walk) arm and exits 9, a
    // stale payload exits with the wrong code.
    let canary = pass_canary(fixture_roster::RUNTIME_CASE_REASSIGNMENT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-case-reassignment-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case reassignment canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case reassignment canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case reassignment canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the second case construction to fully replace the first (exit 70), got {:?} (9 = stale tag took the Walk arm, 72 = no arm matched)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mixed_shape_exit_canary_runs() {
    // MIXED data shape (frozen decision 7): common fields + cases in one
    // declaration. Construction names a common field alongside the payload,
    // a case change zero-initializes the unnamed common field, a common
    // field is read AND written without case knowledge, and tag dispatch
    // binds the payload. Layout: tag at 0, common fields after the tag,
    // payload overlay after the common fields.
    let canary = pass_canary(fixture_roster::RUNTIME_MIXED_SHAPE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-mixed-shape-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("mixed shape canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("mixed shape canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("mixed shape canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected common-field reads/writes and payload binding to agree (exit 70), got {:?} (71 = a dispatch step observed the wrong value)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_array_literal_string_field_exit_canary_runs() {
    // ARRAY-literal struct-element initialization (`let rooms: [Room; 2] =
    // [Room { number: 11, label: "expected" }, ..]`) must write every element
    // field natively. The local-initializer mutation path had a StructLiteral
    // arm but no ArrayLiteral arm, so the whole initializer fell through to
    // the scalar path and selected NOTHING -- scalar element fields read 0 and
    // String descriptors read empty while the interpreter initialized them.
    // Guards read each element's scalar sibling and String field through a
    // runtime index (frame reads, no static folds) plus a cross check that
    // element 0 does not equal element 1's literal.
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_LITERAL_STRING_FIELD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-array-literal-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("array literal string field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("array literal string field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array literal string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected array-literal element init to write scalar and String fields natively (exit 70), got {:?} (71 = a guard observed a zeroed/incorrect element field)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_literal_string_field_exit_canary_runs() {
    // Struct-LITERAL String field initialization (`let msg: T = T { label:
    // "hi" }`) must emit the native descriptor write, same as the assignment
    // form. Data planning previously collected string literals only from
    // assignments / state values / branch targets -- never from `let` local
    // initializers -- so the descriptor-write selection found no data object
    // and silently skipped the write (descriptor stayed zeroed natively).
    // Observed through the wire encoder's bytes plus a case-literal String
    // payload (`Command::Say { text: "ok" }`) destructured and compared.
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_LITERAL_STRING_FIELD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-struct-literal-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct literal string field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("struct literal string field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct literal string field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected literal-form String field init to write the descriptor natively (exit 70), got {:?} (71 = empty/incorrect descriptor observed)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_param_domain_forward_exit_canary_runs() {
    // #66 domain-fact forwarding: an IMMUTABLE `&[u8] in Utf8` parameter, forwarded
    // as a call argument to another domained param, discharges `text in Utf8` via
    // the param's always-holding state invariant (caller-enforced `requires` +
    // immutability). Before the param-domain producer + direct state-invariant
    // consultation this rejected at compile time on the branch-dispatch path
    // (`consume: text in Utf8` saw 0 entry contexts).
    let canary = pass_canary(fixture_roster::RUNTIME_PARAM_DOMAIN_FORWARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("param domain forward canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for a forwarded domained param, got {}",
        outcome.exit_code
    );

    let scratch =
        std::env::temp_dir().join(format!("omega-param-domain-forward-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("param domain forward canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("param domain forward canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("param domain forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded immutable domained param to discharge and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_payload_domain_forward_exit_canary_runs() {
    // #66 case-payload domain forwarding: a local constructed as `Command::First
    // { text: "ok" }` (payload `text: &[u8] in Utf8`) carries `cmd.<payload> in
    // Utf8` -- construction enforcement (#60-1c) proved it. Destructuring the
    // payload and forwarding it (`Command::First { text } -> consume(text)`)
    // discharges `consume: <payload> in Utf8` via the case-payload producer +
    // guarded-transition fallthrough threading. Before, this rejected at compile.
    let canary = pass_canary(fixture_roster::RUNTIME_CASE_PAYLOAD_DOMAIN_FORWARD_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("case payload domain forward canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 for a forwarded case payload, got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-case-payload-domain-forward-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("case payload domain forward canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("case payload domain forward canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("case payload domain forward canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded case payload to discharge and exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_tuple_transition_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TUPLE_TRANSITION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-tuple-transition-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime tuple transition canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime tuple transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime tuple transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(22),
        "expected runtime tuple transition canary to route to tuple arm exit code 22, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_room_use_reentry_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ROOM_USE_REENTRY_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-runtime-room-reentry-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime room use reentry canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime room use reentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime room use reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(41),
        "expected runtime room use reentry canary to route to spent-fountain exit code 41, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_enemy_clear_reentry_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ENEMY_CLEAR_REENTRY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-enemy-reentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime enemy clear reentry canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime enemy clear reentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime enemy clear reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(51),
        "expected runtime enemy clear reentry canary to route to cleared-hall exit code 51, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_clear_carve_render_string_fields_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CLEAR_CARVE_RENDER_STRING_FIELDS_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("runtime clear/carve/render carrier fields canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 198);
    assert_eq!(interpreted.stdout, Vec::<u8>::new());
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-clear-carve-render-string-fields-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "runtime clear/carve/render string fields canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime clear/carve/render string fields canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime clear/carve/render string fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(198),
        "expected cleared then carved room label to render through lookup and exit 198, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_full_level_wrapper_lookup_string_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_FULL_LEVEL_WRAPPER_LOOKUP_STRING_FIELD_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("runtime full-level wrapper carrier lookup canary should check");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 202);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-full-level-wrapper-lookup-string-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out")).expect(
        "runtime full-level wrapper carrier lookup canary should compile from its authored root",
    );

    let executable = compilation.checked_native_executable_path().expect(
        "runtime full-level wrapper carrier lookup canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime full-level wrapper carrier lookup canary should run");

    assert_eq!(
        output.status.code(),
        Some(202),
        "expected full-level wrapper carrier lookup to preserve the room label, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_multi_room_reentry_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MULTI_ROOM_REENTRY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-multi-room-reentry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime multi-room reentry canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime multi-room reentry canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime multi-room reentry canary should run");

    assert_eq!(
        output.status.code(),
        Some(63),
        "expected runtime multi-room reentry canary to preserve all three room flags and exit 63, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_mutable_slice_element_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MUTABLE_SLICE_ELEMENT_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-slice-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable slice write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime mutable slice write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable slice write canary should run");

    assert_eq!(
        output.status.code(),
        Some(21),
        "expected runtime mutable slice write canary to preserve alias mutation and exit 21, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The promoted straight-line sibling: same mutable-slice-view write, but the
// ordinary value helper has NO transitions and delivers a field READ-BACK as
// its terminal value. Guards result delivery end to end through a slice write.

#[test]
fn runtime_mutable_slice_element_write_straight_line_exit_canary_runs() {
    let canary =
        pass_canary(fixture_roster::RUNTIME_MUTABLE_SLICE_ELEMENT_WRITE_STRAIGHT_LINE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-mutable-slice-write-straight-line-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime mutable slice write straight-line canary should compile");

    let executable = compilation.checked_native_executable_path().expect(
        "runtime mutable slice write straight-line canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("runtime mutable slice write straight-line canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the slice-view write to land and the terminal field read-back to exit 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dispatch_mutable_slice_element_write_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DISPATCH_MUTABLE_SLICE_ELEMENT_WRITE_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-dispatch-mutable-slice-write-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime dispatch mutable slice write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime dispatch mutable slice write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime dispatch mutable slice write canary should run");

    assert_eq!(
        output.status.code(),
        Some(31),
        "expected runtime dispatch mutable slice write canary to preserve alias mutation and exit 31, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_indexed_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_INDEXED_READ_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-runtime-array-indexed-read-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("runtime array indexed read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime array indexed read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime array indexed read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.nums[self.i]` (runtime index) to read 20 and 40 and self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_struct_field_write_exit_canary_runs() {
    // A runtime-indexed STRUCT-FIELD write `arr[i].field = v` (array of structs)
    // must invalidate the whole array's folded constants so a later const read
    // `arr[2].field` sees live storage. Regression for the stale-fold that the
    // earlier `arr[i] = v` fix missed (the `Member(Indexed(..))` target shape).
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_STRUCT_FIELD_WRITE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-indexed-struct-field-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("runtime indexed struct-field write canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime indexed struct-field write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime indexed struct-field write canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `entities[i].field = v` then const read-backs to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_particle_system_exit_canary_runs() {
    // A 2D particle system over an array of structs: runtime-indexed struct-field reads
    // and writes, integrating pos += vel each step. Self-checks three cells -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_PARTICLE_SYSTEM_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-particle-system-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("particle system canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("particle system canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("particle system canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the particle system to integrate pos += vel correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_construction_exit_canary_runs() {
    // Nested struct construction `Rect { top_left: Point { .. }, .. }` PANICKED the
    // compiler (an arena span-contiguity assert: the field-value copy appended the
    // inner struct's fields mid-loop, interleaving the outer span). Fixed with
    // reserve-then-set. This canary self-checks the constructed nested fields.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_STRUCT_CONSTRUCTION_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-nested-struct-construct-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested struct construction canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct construction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct construction canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested struct construction (Rect of two Points) to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cross_machine_substate_name_exit_canary_runs() {
    // Two machines each have a `try1` sub-state (Picker::pick, Main::read_at). A named transition
    // target must resolve to a SIBLING state of the CURRENT machine, not collide on the shared
    // name and run the other machine's body. read_at(4) must return table[4]=60 even after
    // pick(2) (whose try1 yields a literal) runs -> exit 70. (Was an interp miscompile.)
    let canary = pass_canary(fixture_roster::RUNTIME_CROSS_MACHINE_SUBSTATE_NAME_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-cross-machine-substate-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("cross-machine substate-name canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("cross-machine substate-name canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cross-machine substate-name canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected read_at(4)=60 despite pick's shared `try1` (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_call_to_array_element_exit_canary_runs() {
    // A single value-call result materializes correctly when written to a const-indexed array
    // element: triple(14)=42 lands at arr[2] with neighbours untouched -> exit 70. The working
    // write-side contrast to the value-call dispatch-position drop and the multi-call shared slot.
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_TO_ARRAY_ELEMENT_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-vc-array-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("value-call to array element canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call to array element canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call to array element canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected triple(14)=42 written to arr[2] with neighbours 0 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_computed_transition_args_exit_canary_runs() {
    // Computed values (an addition, a subtraction, a cast) passed directly as transition
    // arguments materialize correctly. chk(7+3, 7-3, 300 as u8) sees sum=10, diff=4, byte=44
    // -> exit 70. The working contrast to the value-call-as-transition-arg silent drop.
    let canary = pass_canary(fixture_roster::RUNTIME_COMPUTED_TRANSITION_ARGS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-computed-args-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("computed transition args canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("computed transition args canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("computed transition args canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected computed transition args (sum 10, diff 4, byte 44) to materialize (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_by_value_param_exit_canary_runs() {
    // Passing a struct BY VALUE into a value-machine and reading all its fields in distinct
    // positional weights. decode(Coeffs{1,2,3}) = 1*100 + 2*10 + 3 = 123 -> exit 70. Pins
    // the working envelope around task #15 (scalar fields of a by-value struct param resolve).
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_BY_VALUE_PARAM_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-by-value-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct by-value param canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct by-value param canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct by-value param canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected by-value struct param decode to yield 123 (exit 70); got {:?} (the decoded value on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_call_composition_exit_canary_runs() {
    // Function composition: chaining value-machine calls so each result feeds the next.
    // add_ten(5)=15, double(15)=30, minus_five(30)=25 -> exit 70. (Sequential binding; the
    // nested form f(g(x)) is a clean error today, documented in the canary.)
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_COMPOSITION_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-composition-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("value call composition canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value call composition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value call composition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected three-stage value-call pipeline to yield 25 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_value_call_exit_canary_runs() {
    // A value-machine that computes and RETURNS a struct (product type), completing the
    // value-call return-type map alongside scalars and sum-type returns. stats(7,3) returns
    // a record whose two independently-computed fields are 10 and 4 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_VALUE_CALL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-value-call-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct value call canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct value call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-call to return a record with sum 10 and diff 4 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_option_value_call_exit_canary_runs() {
    // A value-machine that RETURNS an Option (Some/None), called in a loop with each result
    // matched -- the idiomatic functional shape for find/lookup/parse. classify(x) over
    // [5,-3,7] yields two present values and one absent; the present values sum to 12 and
    // one absent is counted -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_OPTION_VALUE_CALL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-option-value-call-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("option value call canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("option value call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("option value call canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Option-returning value-call to sum Somes=12 and count 1 None (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_result_match_exit_canary_runs() {
    // Result-style error handling at runtime: a two-case enum (Ok/Err) produced
    // conditionally, then matched and handled in a loop. Safe-dividing 10/2, 7/0, 20/4
    // sums the Ok values to 10 and counts 1 Err -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_RESULT_MATCH_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-result-match-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("result match canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("result match canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("result match canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Ok/Err handling to sum Oks=10 and count 1 Err (exit 70); got {:?} (the sum on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_entity_component_exit_canary_runs() {
    // An array of entities each holding a nested component struct (the entity-component
    // pattern): runtime-indexed access through a member path (`self.ents[i].pos.x`) read in
    // a loop and temp-RMW written back. Three entities pos.x = 1,2,3: sum 6, doubled to
    // 2,4,6 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_ENTITY_COMPONENT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-entity-component-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("entity component canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("entity component canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("entity component canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the entity-component array (sum 6, doubled nested fields) to self-check (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_state_machine_exit_canary_runs() {
    // A state machine whose state lives in nested structs: a nested-vs-nested guard
    // subject, nested-field RMW, a cross-struct write, and a two-way nested verify. The
    // runtime-indexed-ARRAY guard bug does NOT extend to member paths -- these resolve the
    // correct field. Sums 1..5 = 15 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_STRUCT_STATE_MACHINE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-struct-sm-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested struct state machine canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct state machine canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct state machine canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested-struct state machine to sum 1..5 = 15 (exit 70); got {:?} (a non-70 code is the bad sum)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_array_element_struct_copy_exit_canary_runs() {
    // Value semantics through an array-element struct copy: `self.f = self.arr[1]` produces an
    // independent copy, so mutating f leaves arr[1] untouched. Discriminates both ways: arr[1]
    // keeps (5,6), f holds the mutated (50,60) -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_ELEMENT_STRUCT_COPY_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-arr-elem-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("array-element struct copy canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("array-element struct copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array-element struct copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected array-element struct copy to be independent (arr[1] unchanged, exit 70); got {:?} (the aliased value on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_value_semantics_exit_canary_runs() {
    // Deep nesting + whole-struct value semantics: a 3-level nested field read AND
    // write, a whole-struct copy by assignment, and copy independence (overwriting the
    // source leaves the copy intact). The data backbone of serious apps.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_STRUCT_VALUE_SEMANTICS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-struct-value-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested struct value-semantics canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct value-semantics canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct value-semantics canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 3-level nesting + whole-struct copy + copy independence to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_array_literal_exit_canary_runs() {
    // Composite literal nesting: a struct literal with an array-literal field AND an
    // array-of-struct-literals field. Guards the expression-handle + struct-field copy
    // paths near the nested-struct panic fix. Self-checks the constructed values.
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_ARRAY_LITERAL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-array-literal-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct-array literal canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-array literal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-array literal canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a struct literal with array + struct-array fields to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_enum_struct_payload_exit_canary_runs() {
    // An enum variant with a STRUCT-typed payload `Event::Click(at: Point, ..)`. The
    // payload field's named type symbol was never resolved (the resolution pass
    // skipped variant payload fields), so the layout builder errored. Now construct +
    // match + read the struct payload's fields.
    let canary = pass_canary(fixture_roster::RUNTIME_ENUM_STRUCT_PAYLOAD_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-enum-struct-payload-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("enum struct-payload canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("enum struct-payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum struct-payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected enum variant with a struct payload to construct/match/extract (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_copy_sum_array_receiver_exit_canary_interprets_and_establishes_its_entry() {
    // The entry receiver holds `[Token; 16]`, a fixed array of a copy sum with
    // common fields and payload cases (the product lexer's `[Token; 16384]`
    // shape). Unit planning admits the array element like a copy record, so a
    // host-target checked compile selects the attached entry and rejoins its
    // attachment identity; the interpreter oracle folds the three elements to
    // 70. Native production is not claimed: the element store and element
    // load statements are separate Unit slices.
    let canary = pass_canary(fixture_roster::RUNTIME_COPY_SUM_ARRAY_RECEIVER_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("copy-sum-array receiver canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should fold the copy-sum array to 70, got {} ({:?})",
        outcome.exit_code, outcome.error
    );

    let established = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some(crate::native_hosted_target()),
    ))
    .expect("the host target selects the attached entry over the copy-sum array receiver");
    assert!(
        established.selected_program_entry_machine().is_some(),
        "ProgramEntry establishment rejoins the receiver's attachment identity"
    );
}

#[test]
fn runtime_enum_classify_dispatch_exit_canary_runs() {
    // A value-machine returns an enum computed through nested runtime guards (a sign classifier),
    // dispatched by a multi-arm match. All three classifications (Pos/Neg/Zero) are checked, so a
    // wrong classify arm or a wrong dispatch arm takes the false path. Value-call enum return +
    // multi-arm enum dispatch, in one program -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_ENUM_CLASSIFY_DISPATCH_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-enum-classify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("enum classify-dispatch canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("enum classify-dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum classify-dispatch canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-call enum classification + multi-arm dispatch to route all three signs (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_field_accumulate_loop_exit_canary_runs() {
    // Two-level nested struct fields (`self.body.pos.x`) mutated in place across a
    // state-machine loop -- the physics/entity-update pattern (position += velocity).
    // Two sibling nested fields must track independently (pos.x -> 70, pos.y -> 30),
    // chained-guard self-checked. Guards against nested-place read/write cross-talk.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_FIELD_ACCUMULATE_LOOP_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-accum-loop-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("nested-field accumulate-loop canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("nested-field accumulate-loop canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested-field accumulate-loop canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nested struct fields to accumulate independently across a loop (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_write_const_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_WRITE_CONST_READ_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-indexed-write-const-read-{}",
        std::process::id()
    ));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("indexed-write/const-read canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-write/const-read canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-write/const-read canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a runtime-indexed write to invalidate whole-array constants so const-indexed reads see live storage (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn zero_length_byte_array_is_admitted_at_check() {
    // `[u8; 0]` is a first-class empty value: it is admitted in locals,
    // constants, parameters, returns, record fields, and nested arrays, and it
    // is constructed exactly by `[]` or `""`. The use-site fences (no provable
    // index, exact-length literals) are pinned by
    // fail/data/zero_length_byte_array_*.
    let canary = pass_canary(fixture_roster::ZERO_LENGTH_BYTE_ARRAY_ADMISSION);
    check_canary(&canary).expect("a zero-length byte array should be admitted at check");
}

#[test]
fn zero_length_byte_array_use_fences_reject_at_check() {
    // `[u8; 0]` is admitted, but using it is fenced: no element exists so no
    // index is provable, and construction must supply exactly 0 elements or
    // bytes. Each fixture carries its pinned diagnostic in expected.txt.
    for &name in fixture_roster::ZERO_LENGTH_BYTE_ARRAY_USE_FENCE_FAIL_CANARIES {
        let canary = fail_canary(name);
        let expected_fragment = fs::read_to_string(canary.join("expected.txt"))
            .expect("fail canary should carry expected.txt")
            .trim()
            .to_owned();
        let diagnostics = match check_canary(&canary) {
            Ok(()) => panic!("{name} must reject at check"),
            Err(diagnostics) => diagnostics,
        };
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(&expected_fragment),
            "{name} missing expected fragment {expected_fragment:?}:\n{combined}"
        );
    }
}

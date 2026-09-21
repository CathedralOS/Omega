use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_canary_without_output,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, executable_name,
    fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_enum_match_breadth_exit_canary_runs() {
    // Enum matching breadth + the SOUND pattern for a runtime-indexed enum element
    // (bind to a local first). grid[2]=Goal (non-first variant) and a field-name
    // collision (Potion.power vs Weapon.power) self-check to exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_ENUM_MATCH_BREADTH_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-enum-match-breadth-{}", std::process::id()));
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("enum match breadth canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("enum match breadth canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum match breadth canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexed-via-local enum match + payload extraction to self-check (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_conformance_item_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONFORMANCE_ITEM_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-conformance-item-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("conformance item canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("conformance item canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("conformance item canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `Circle satisfies Shape;` to validate against the written member and run unchanged (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_record_equality_exit_canary_runs() {
    // Equatable synthesis (decisions 8 + 11): `Point satisfies Equatable;`
    // makes `==`/`!=` on the record structural -- equal values match, one
    // differing middle field misses.
    let canary = pass_canary(fixture_roster::EQUATABLE_RECORD_EQUALITY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-record-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable record equality canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable record canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable record equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==`/`!=` on `Point` to compare field by field (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_erased_field_record_exit_canary_runs() {
    // Synthesized `==`/`!=` compares only RELEVANT fields: `proof [erased]`
    // contributes no runtime read, and a case whose payload is entirely
    // erased compares by tag alone.
    let canary = pass_canary(fixture_roster::EQUATABLE_ERASED_FIELD_RECORD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-erased-field-record-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable erased-field record canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable erased-field record canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable erased-field record canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==`/`!=` to skip `[erased]` fields (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_sum_payload_equality_exit_canary_runs() {
    // Equatable synthesis on a payload-bearing sum: tag equality AND the
    // matching case's payload fields. Same-case-equal matches; same-case-
    // different-payload and different-case miss; the constructed-literal
    // compare pins the single-arm form.
    let canary = pass_canary(fixture_roster::EQUATABLE_SUM_PAYLOAD_EQUALITY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-sum-payload-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable sum payload equality canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable sum payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable sum payload equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `Command` to compare tag AND payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_mixed_shape_equality_exit_canary_runs() {
    // Equatable synthesis on a MIXED shape: common fields AND tag AND the
    // matching case's payload. The second compare differs ONLY in a common
    // field (the reconstruction zero-initialized it), so equality that skips
    // common fields exits 71. Also regression net for the boolean-folding
    // factor/distribute mutual recursion this expansion first exposed.
    let canary = pass_canary(fixture_roster::EQUATABLE_MIXED_SHAPE_EQUALITY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-mixed-shape-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable mixed shape equality canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable mixed shape canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable mixed shape equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `RoomEvent` to compare common fields AND tag AND payload (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_string_field_equality_exit_canary_runs() {
    // Equatable synthesis over a String field: `==` compares text CONTENT
    // (length AND bytes) through the value-position text-equals operand --
    // equal contents match; same-length-different-bytes, different-length,
    // and equal-text-different-scalar-sibling all miss.
    let canary = pass_canary(fixture_roster::EQUATABLE_STRING_FIELD_EQUALITY_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-string-field-equality-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable string field equality canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable string field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable string field equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==` on `Tag` to compare String content (length AND bytes) plus the scalar sibling (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_string_not_equals_exit_canary_runs() {
    // Equatable `!=` over a String-bearing record in VALUE position: the
    // simplifier De-Morgans `equality == false` into per-field `!=` compares,
    // so the String term lowers as the negated text-equals leaf
    // (`text_equals(..) == 0`). The names differ ("gold" vs "iron") while the
    // scalar siblings are equal, so dropping the String term (the old
    // miscompile: the whole initializer write silently vanished and the ZII
    // false took the bad arm) flips the exit code.
    let canary = pass_canary(fixture_roster::EQUATABLE_STRING_NOT_EQUALS_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-string-not-equals-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable string not-equals canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable string inequality canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable string not-equals canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `!=` on `Tag` to see the differing String content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_string_equality_guard_exit_canary_runs() {
    // Equatable structural `==` over a String-bearing record DIRECTLY in
    // GUARD position: the conjunction's String clause routes through the
    // value-position TextEquals content compare (the raw 16-byte descriptor
    // place compare cannot encode), the scalar clause stays a place compare.
    // Equal contents take the `true` arm (exit 70).
    let canary = pass_canary(fixture_roster::EQUATABLE_STRING_EQUALITY_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-string-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable string guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable string guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable string guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guard-position structural `==` on `Tag` to compare String content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn equatable_qualified_field_reference_exit_canary_runs() {
    // Equatable conformance on a record whose field type is referenced by a
    // module-qualified spelling (`leaf::region::Region`): synthesized
    // structural equality must join the resolved type symbol to its declared
    // name, so the field's conformance is found and `Filter == Filter`
    // expands over the nested record. Qualified `==` operands in a machine
    // body take the same route (exit 70).
    let canary = pass_canary(fixture_roster::EQUATABLE_QUALIFIED_FIELD_REFERENCE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-qualified-field-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable qualified-field canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("equatable qualified-field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("equatable qualified-field canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected synthesized structural `==`/`!=` over a module-qualified field type to agree (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_deep_nested_field_exit_canary_runs() {
    // A 5-level nested field chain (self.l1.l2.l3.l4.v) written and read back; offsets must compose
    // through every level. v + w = 30 + 40 = 70 (a sibling `tag` decoy discriminates the offsets).
    let canary = pass_canary(fixture_roster::RUNTIME_DEEP_NESTED_FIELD_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-deep-nested-field-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("deep nested field canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("deep nested field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("deep nested field canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 5-level nested field access to resolve correctly (30+40 == 70, exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_value_copy_exit_canary_runs() {
    // Struct assignment is a value copy, not an alias: copy a->b, mutate a, b stays unchanged;
    // same between array-of-structs elements. Both sums stay 14 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_VALUE_COPY_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-value-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.join("out"))
        .expect("struct value copy canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct value copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct value copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected struct + array-element value copies to stay unchanged after mutating the source (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_whole_struct_mutation_copy_canary_runs() {
    // The first migrated CopyPlaces sites (Place rung 2a): cross-region
    // field writes + a same-region whole-struct copy, relocations patched
    // BY PLACE REGION from the materializer's site list.
    let canary = pass_canary(fixture_roster::RUNTIME_WHOLE_STRUCT_MUTATION_COPY_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("whole-struct mutation copy canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (both fields survive the copy), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-copy-places-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("whole-struct mutation copy canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("whole-struct mutation copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("whole-struct mutation copy canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the CopyPlaces mutation copies to deliver (exit 70), got {:?} (71 = a base patched to the wrong region's symbol)
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_data_properties_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DATA_PROPERTIES_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-runtime-data-properties-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("data properties canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("data properties canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("data properties canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `[copy]` declarations to verify and run identically to property-free data (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn case_first_payload_zero_established_canary_compiles() {
    let canary = pass_canary(fixture_roster::CASE_FIRST_PAYLOAD_ZERO_ESTABLISHED);

    compile_canary_without_output(&canary)
        .expect("a zero-established first-case payload should compile");
}

#[test]
fn compound_assignment_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::COMPOUND_ASSIGNMENT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-compound-assignment-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("compound assignment canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("compound assignment canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("compound assignment canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `+= -= *= /= %=` to chain correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_chained_field_mutation_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CHAINED_FIELD_MUTATION_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-chained-field-mutation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("chained field mutation canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("chained field mutation canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("chained field mutation canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected chained read-modify-write to observe prior writes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_comparison_guard_signedness_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_COMPARISON_GUARD_SIGNEDNESS_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-comparison-guard-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("comparison guard signedness canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("comparison guard signedness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("comparison guard signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected guard comparisons to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_comparison_value_signedness_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_COMPARISON_VALUE_SIGNEDNESS_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-comparison-value-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("comparison value signedness canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("comparison value signedness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("comparison value signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected value-position comparisons to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_min_max_signedness_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MIN_MAX_SIGNEDNESS_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-min-max-signedness-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("min/max signedness canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("min/max signedness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min/max signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected min/max to respect operand signedness (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_division_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_UNSIGNED_DIVISION_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-unsigned-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned division canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned division canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned division/remainder/logical-shift on high-bit u32 (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_min_max_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_UNSIGNED_MIN_MAX_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-unsigned-min-max-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned min/max canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned min/max canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned min/max canary should run");

    assert_eq!(
        output.status.code(),
        Some(88),
        "expected max(u64::MAX, 5)==u64::MAX and min==5 (unsigned witness) to exit 88, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_modulo_call_argument_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_UNSIGNED_MODULO_CALL_ARGUMENT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-unsigned-modulo-call-argument-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("unsigned modulo call-argument canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned modulo call-argument canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned modulo call-argument canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the inline `raw % 100` call argument to use UNSIGNED modulo \
         so the dispatch ladder selects the satisfied arm (exit 70 = 3 RNG \
         draws, interpreter semantics; exit 71 = the signed-remainder misfire \
         routed the second event into the enemy arm and drew once extra -- the \
         dungeon seed-7 14-vs-15 residual), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_named_conversion_alias_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_NAMED_CONVERSION_ALIAS_EXIT);
    let main_path = canary.join("main.omg");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-nested-named-conversion-alias-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("nested named-conversion alias canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("nested named-conversion alias canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the nested conversion to read the caller's RandomState alias \
         and return its high word (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_unsigned_modulo_cast_operand_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_UNSIGNED_MODULO_CAST_OPERAND_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-unsigned-modulo-cast-operand-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("unsigned modulo cast-operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned modulo cast-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned modulo cast-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `((random.seed >> 32) as u32) % 199` to use UNSIGNED modulo \
         (the cast's TARGET type decides operand signedness; exit 70 = roll 158, \
         interpreter semantics; exit 71 = the signed-remainder misfire stored \
         -87 in the u32 slot), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn saturating_multiply_overflow_both_signs_canary_runs() {
    // Saturating i32 multiply overflow clamps to the SIGN-CORRECT bound: positive
    // overflow -> INT_MAX, negative overflow -> INT_MIN (clamping negative to
    // INT_MAX is the classic bug). exit 72 = positive wrong; 73 = negative bound.
    let canary = pass_canary(fixture_roster::SATURATING_MULTIPLY_OVERFLOW_BOTH_SIGNS);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("saturating multiply canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (+overflow->INT_MAX, -overflow->INT_MIN), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-sat-mul-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating multiply canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating multiply canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating multiply canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sign-correct saturating multiply clamp (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn saturating_signed_divide_min_by_neg_one_canary_runs() {
    // Saturating signed divide/modulo of TYPE_MIN by -1: the same idiv #DE corner
    // as the Wrapping case, but CLAMPED (INT_MIN / -1 -> INT_MAX, % -> 0).
    // Verifies append_saturating_signed_divide_modulo's -1 guard. exit 72 = divide
    // did not clamp; 73 = modulo not 0.
    let canary = pass_canary(fixture_roster::SATURATING_SIGNED_DIVIDE_MIN_BY_NEG_ONE);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("saturating INT_MIN/-1 canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (INT_MIN/-1 clamps to INT_MAX, %0), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-sat-div-min-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating INT_MIN/-1 canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating INT_MIN/-1 canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating INT_MIN/-1 canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected INT_MIN/-1 to clamp to INT_MAX (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn wrapping_signed_divide_min_by_neg_one_canary_runs() {
    // Wrapping signed divide/modulo of TYPE_MIN by -1: x86 `idiv` raises #DE
    // (integer-overflow) for this corner, so the Wrapping domain guards it and
    // produces the wrapped result (INT_MIN / -1 -> INT_MIN, INT_MIN % -1 -> 0).
    // Before the guard the native binary crashed with STATUS_INTEGER_OVERFLOW.
    // exit 72 = divide did not wrap; 73 = modulo not 0.
    let canary = pass_canary(fixture_roster::WRAPPING_SIGNED_DIVIDE_MIN_BY_NEG_ONE);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("wrapping INT_MIN/-1 canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (INT_MIN/-1 wraps to INT_MIN, %0), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!("omega-wrap-div-min-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wrapping INT_MIN/-1 canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("wrapping INT_MIN/-1 canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wrapping INT_MIN/-1 canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected INT_MIN/-1 to wrap (exit 70), got {:?} (a crash would be a large negative code = idiv #DE)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_signed_division_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SIGNED_DIVISION_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-signed-division-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("signed division canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("signed division canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("signed division canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed division/remainder of a negative dividend (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_shift_right_signedness_canary_runs() {
    // Right-shift is signedness-sensitive: a signed operand must lower to `sar`
    // (arithmetic), an unsigned operand to `shr` (logical). `-8 >> 1 == -4` AND
    // `0xFFFFFFFE >> 1 == 0x7FFFFFFF` both hold only when the two shifts pick
    // different instructions. exit 71 = a `sar` misfire on the unsigned value
    // (0xFFFFFFFF instead of 0x7FFFFFFF). Values are field-held so they are
    // genuine runtime operands (instruction selection resolves the field's
    // signedness); the const-folded high-bit case is a separate documented gap.
    let canary = pass_canary(fixture_roster::RUNTIME_SHIFT_RIGHT_SIGNEDNESS);
    let main_path = canary.join("main.omg");

    // Interpreter oracle first: it must agree the exit is 70.
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("shift-right signedness canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should exit 70 (sar for signed, shr for unsigned), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-shift-right-signedness-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shift-right signedness canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shift-right signedness canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shift-right signedness canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed `sar` AND unsigned `shr` right shifts (exit 70), got {:?} \
         (71 = unsigned >> emitted `sar`)\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_saturating_narrow_canary_runs() {
    // CM3 differential legs: fold_landed's Saturating CLAMP at NARROW widths
    // (i8 clamps to 127/-128, u8 to 255; the division folds unsigned at u8).
    // exit 71 = a fold regressed to the bare-i64 window (no clamp).
    let canary = pass_canary(fixture_roster::CONST_FOLD_SATURATING_NARROW_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("saturating narrow const-fold canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should clamp narrow saturating folds (exit 70), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-const-fold-sat-narrow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating narrow const-fold canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating narrow const-fold canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating narrow const-fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected narrow saturating clamps at fold (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn const_fold_wrapping_narrow_canary_runs() {
    // CM3 differential legs: fold_landed's wrap-to-width face at NARROW
    // widths (i8: 100+100 -> -56; u16: 65535+2 -> 1). exit 71 = a fold
    // regressed to the bare-i64 window (no wrap).
    let canary = pass_canary(fixture_roster::CONST_FOLD_WRAPPING_NARROW_EXIT);
    let main_path = canary.join("main.omg");

    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("wrapping narrow const-fold canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should wrap narrow folds to width (exit 70), got {}",
        outcome.exit_code
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-const-fold-wrap-narrow-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wrapping narrow const-fold canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wrapping narrow const-fold canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wrapping narrow const-fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected narrow wrapping folds at width (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

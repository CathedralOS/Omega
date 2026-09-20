use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{compile_rooted_canary_for_native_host, fs, pass_canary};

#[test]
fn runtime_version_migration_exit_canary_runs() {
    // Historical and current shapes are ordinary data. The explicit migration
    // machine lands both current-shape writes and exits 70.
    let canary = pass_canary(fixture_roster::RUNTIME_VERSION_MIGRATION_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-version-migration-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("version migration canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "version-migration canary",
        "the explicit migration should land both current-shape writes",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_versioned_match_zii_exit_canary_runs() {
    // An explicitly constructed ordinary lineage sum selects its V1 case even
    // though Current is written first; the historical payload drives exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_VERSIONED_MATCH_ZII_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-versioned-match-zii-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("versioned match canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "versioned-match canary",
        "the explicitly constructed lineage sum should select its v1 arm",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_versioned_three_era_match_zii_exit_canary_runs() {
    // An ordinary sum over three explicit era shapes selects V1 even though
    // both newer cases are written first; the V1 payload drives exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_VERSIONED_THREE_ERA_MATCH_ZII_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-versioned-three-era-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("three-era versioned match canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "three-era versioned-match canary",
        "the three-era lineage sum should select its v1 arm",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_equatable_scalar_not_equals_guard_exit_canary_runs() {
    // Equatable `!=` negation + `==` of a scalar record DIRECTLY in guard
    // position (the other equatable canaries route compares through a `let`).
    // String-bearing variants: pass/traits/equatable_string_not_equals_exit
    // (value position) and pass/traits/equatable_string_equality_guard_exit
    // (guard position).
    let canary = pass_canary(fixture_roster::RUNTIME_EQUATABLE_SCALAR_NOT_EQUALS_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-equatable-scalar-neq-guard-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("equatable scalar != guard canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "equatable scalar guard canary",
        "scalar-record equality and inequality should drive every guard rung",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_case_membership_mixed_shape_exit_canary_runs() {
    // Decision 11 `in` membership over a MIXED shape (decision 7): common
    // fields live between the tag and the payload overlay, so the membership
    // test must stay tag-only -- and survive a common-field write.
    let canary = pass_canary(fixture_roster::RUNTIME_CASE_MEMBERSHIP_MIXED_SHAPE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-membership-mixed-shape-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("mixed-shape membership canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "mixed-shape case-membership canary",
        "case membership should remain tag-only across common-field writes",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_repeated_max_one_exit_canary_runs() {
    // Wire exact-array field with the DEGENERATE extent `[u32; 1]`: one
    // required element, no synthetic count, and packed framing round-trips.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_REPEATED_MAX_ONE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-repeated-max-one-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("repeated max-one wire canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "max-one repeated-field wire canary",
        "the required single-element packed field should roundtrip",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_utf8_exit_canary_runs() {
    // &[u8]-in-Utf8 wire decode roundtrips for honest bytes (the
    // adversarial half is the pinned soundness hole).
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_UTF8_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wireutf8-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 wire roundtrip canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "Utf8 wire-roundtrip canary",
        "honest Utf8 bytes should roundtrip through the wire codec",
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_utf8_edge_verdicts_exit_canary_runs() {
    // The utf8 validator's edge classes: honest multi-byte SOUND; overlong /
    // surrogate / beyond-max / truncated all INVALID.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_UTF8_EDGE_VERDICTS_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-utf8edge-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 edge canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "Utf8 edge-verdict canary",
        "the validator should agree on every honest and invalid Utf8 edge class",
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_utf8_invalid_refused_exit_canary_runs() {
    // Adversarial 0xFF 0xFF refuses with verdict Invalid on every engine.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_UTF8_INVALID_REFUSED_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-utf8ref-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("utf8 refusal canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "Utf8 invalid-refusal canary",
        "the adversarial byte pair should produce the Invalid verdict",
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn wire_preserving_decode_relay_exit_canary_runs() {
    // First authored `PreservingDecode` that executes: the decoder validates
    // the known member, binds the unknown tail into `OpaqueWireRemainder`
    // under the codec identity, and the consumer reads both halves back.
    let canary = pass_canary(fixture_roster::WIRE_PRESERVING_DECODE_RELAY_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-preserving-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("preserving-decode canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "preserving-decode canary",
        "the decoded value and the retained opaque remainder should both agree with the frame",
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_schema_as_value_type_exit_canary_runs() {
    // A numbered data serves as a plain program type + encodes from itself.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_SCHEMA_AS_VALUE_TYPE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-schemaval-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("schema-as-value canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "wire schema-as-value canary",
        "the numbered data should remain usable as its ordinary program value type",
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_let_compare_exit_canary_runs() {
    // A let-bound comparison of a decoded field reads the DECODED value
    // (the wire selection clears the static-value table).
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_LET_COMPARE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-declc-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("decode-let-compare canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "wire decode-let comparison canary",
        "the let-bound comparison should read the decoded field value",
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_encode_repeated_then_string_exit_canary_runs() {
    // Wire repeated + String-last in one message: two runtime-sized appends
    // in sequence -- the String's cursor must start where the packed payload
    // actually ended. Encode-only (String decode has not landed); the exact
    // 10 bytes are asserted in-program.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_REPEATED_THEN_STRING_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-repeated-string-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("repeated-then-string wire canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "wire repeated-then-string canary",
        "the String cursor should begin after the packed repeated payload",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_nested_and_repeated_exit_canary_runs() {
    // Wire nested message + repeated field in ONE message: both stage
    // runtime-sized payloads, so the composition pins cursor handoff between
    // them on both the encode and decode sides (written = read = 13).
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_NESTED_AND_REPEATED_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-nested-repeated-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested-and-repeated wire canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "wire nested-and-repeated roundtrip canary",
        "the nested message and packed repeated field should share exact cursor handoff",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_const_array_length_transitive_exit_canary_runs() {
    // Comptime stage 1, transitive admission: the const-position callee CALLS
    // another build-time-admissible machine (base() * 3 + 1 = 16), pinning
    // that const evaluation runs the call machinery, not just expression folding.
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_ARRAY_LENGTH_TRANSITIVE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-const-length-transitive-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("transitive const length canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "transitive const-array length canary",
        "the transitively evaluated length should size the exact 16-slot array",
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_const_array_length_bare_call_arm_exit_canary_runs() {
    // Comptime: the const-position callee's value arm is a PARENTHESIZED
    // BARE CALL (`_ -> (burn(4, 12))`). The parenthesized lone call is a
    // value expression (not a transition target), so const evaluation
    // resolves the free machine `burn` like the arithmetic-wrapped spelling
    // does: 16 slots, both the write and the index-15 typecheck land.
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_ARRAY_LENGTH_BARE_CALL_ARM_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-const-length-bare-call-arm-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bare-call-arm const length canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bare-call-arm const-array length canary",
        "the parenthesized bare call should size the exact 16-slot array",
    );

    let _ = fs::remove_dir_all(&scratch);
}

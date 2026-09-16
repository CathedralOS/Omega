use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, check_canary,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, executable_name,
    fail_canary, fs, interpret, pass_canary, production_compile, unique_no_output_build_dir,
};
use compiler::CheckedCompileRequest;

#[test]
fn ordinary_numbered_record_codecs_check_and_interpret_exact_bytes() {
    for fixture in [
        fixture_roster::RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT,
        fixture_roster::RUNTIME_WIRE_ROUNDTRIP_PRIMITIVE_EXIT,
    ] {
        let canary = pass_canary(fixture);
        let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
            &canary.join("main.omg"),
            None,
        ))
        .unwrap_or_else(|diagnostics| panic!("{fixture}: {diagnostics:#?}"));
        let outcome = interpret(&checked, &[]);
        assert_eq!(outcome.error, None, "{fixture}");
        assert_eq!(outcome.exit_code, 70, "{fixture}");
    }
}

#[test]
fn numbered_decoder_does_not_ignore_record_domain_obligations() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_PRIMITIVE_EXIT);
    let source = fs::read_to_string(canary.join("main.omg"))
        .expect("roundtrip source")
        .replace(
            "data CounterMessage {",
            "data CounterMessage where counter <= 1000 {",
        )
        .replace(
            "CounterMessage::encode(&msg, &mut self.buffer, &mut self.written);",
            "",
        );
    let scratch =
        std::env::temp_dir().join(format!("omega-numbered-domain-{}", std::process::id()));
    fs::create_dir_all(&scratch).expect("scratch directory");
    let main_path = scratch.join("main.omg");
    fs::write(&main_path, &source).expect("constrained numbered source");
    fs::copy(canary.join("build.omg"), scratch.join("build.omg"))
        .expect("roundtrip build declaration");
    let diagnostics =
        compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
            .expect_err("implicit codecs cannot discard whole-record obligations");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("remained unresolved")
                && diagnostic
                    .source_span
                    .is_some_and(|span| &source[span.span.start..span.span.end] == "decode")
        }),
        "codec rejection, not an unrelated source error: {diagnostics:#?}"
    );
    fs::remove_dir_all(scratch).expect("scratch cleanup");
}

#[test]
fn runtime_method_view_write_after_last_use_exit_canary_runs() {
    // Lifetimes stage 1, NLL complement of
    // fail/borrow/method_view_receiver_unrelated_field_write: the
    // receiver-wide loan of a method-returned view ends at the view's LAST
    // USE, so a later write to another field of the same receiver compiles
    // and both writes land (7 + 63 = 70).
    let canary = pass_canary(fixture_roster::RUNTIME_METHOD_VIEW_WRITE_AFTER_LAST_USE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-method-view-after-last-use-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("method-view write-after-last-use canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("method-view write-after-last-use canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("method-view write-after-last-use canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the view write then the post-loan field write to both land (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_view_of_view_chain_exit_canary_runs() {
    // Lifetimes stage 1: CHAINED view-of-view through two free machines
    // (pick -> &mut Cell, narrow -> &mut i32). The elision linkage composes
    // and the two-hop write lands in the root machine-owned storage.
    let canary = pass_canary(fixture_roster::RUNTIME_VIEW_OF_VIEW_CHAIN_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-view-of-view-chain-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("view-of-view chain canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("view-of-view chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("view-of-view chain canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the write through the chained leaf view to reach the root array element (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn unchanged_slice_length_ranking_rejects_in_checked_semantics() {
    let canary = fail_canary(fixture_roster::TERMINATION_SLICE_LENGTH_ORDER_UNIMPLEMENTED);
    let diagnostics = check_canary(&canary)
        .expect_err("an unchanged Slice::Length ranking must reject before lowering");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot prove the `terminates by` ranking"),
        "expected the unchanged Slice::Length ranking to be rejected, got:\n{combined}"
    );
}

#[test]
fn runtime_shrinking_slice_recursion_exit_canary_runs() {
    // Self-recursive dispatch with threaded scalar arguments over a shrinking
    // slice: `self.accumulate(items[1..], items[0].value)` retargets the SAME
    // frame slots it reads (a self-recursive machine shares one call context),
    // so the transition must stage the subslice descriptor AND read the head
    // element THROUGH the old descriptor before committing either. A past bug
    // resolved `items[0].value` as a plain place over the descriptor slot,
    // handing `take` the data pointer's low bytes instead of the element.
    // 10+20+15+25 threaded one step behind sums to 70 in machine state.
    let canary = pass_canary(fixture_roster::RUNTIME_SHRINKING_SLICE_RECURSION_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-shrinking-slice-recursion-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("shrinking slice recursion canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shrinking slice recursion canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shrinking slice recursion canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the threaded scalar accumulation over the shrinking slice to total 70, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_encode_primitive_exit_canary_runs() {
    // Wire stage 2a: `CounterMessage::encode(&msg, &mut self.buffer,
    // &mut self.written)` frames the schema's CURRENT era in compact_binary
    // v0 -- era varint, then per field in field-number order a tag varint and
    // a value varint (LEB128; signed values zigzag; bool 0/1). The canary
    // checks the eight expected bytes (hand-computed in its header comment)
    // and the written count in-language; exits 70 when byte-exact.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_PRIMITIVE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wire-encode-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire encode canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire encode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire encode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 encoder to produce the hand-computed bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn numbered_case_identities_compile() {
    let canary = pass_canary(fixture_roster::NUMBERED_CASE_IDENTITIES);
    let build_dir =
        std::env::temp_dir().join(format!("omega-numbered-cases-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = production_compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::Check,
    })
    .expect("numbered case identities should survive the compiler pipeline");
    assert!(!compilation.wrote_output());
    let report = fs::read_to_string(build_dir.join("04_wire_protocols.txt"))
        .expect("identity-keyed ordinary data should appear in the wire artifact");
    assert!(
        report.contains("## data Lookup")
            && report.contains("#1 Found payload:")
            && report.contains("#1 value: T")
            && report.contains("retired payload identities: #2")
            && report.contains("retired case identities: #3")
            && report.contains("normalized schema identity: 0x")
            && !report.contains("Lookup::encode"),
        "ordinary case identities and tombstones must remain visible in the artifact:\n{report}"
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_wire_roundtrip_primitive_exit_canary_runs() {
    // Wire stage 2b: encode { counter: 300, delta: -2, flag: true } into
    // [0x00, 0x00, 0xAC, 0x02, 0x01, 0x03, 0x02, 0x01] (hand-computed in the
    // canary header), then `decode(&mut decoded, &buffer, &mut read,
    // &mut ok)` reads the same 8 bytes back: ok = true, read = 8, and every
    // decoded field equals the original (zigzag round-trips -2). Exits 70 on
    // a full match.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_PRIMITIVE_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wire-roundtrip-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire roundtrip canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire roundtrip canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 decoder to round-trip the encoded message (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_ranged_field_exit_canary_runs() {
    // Hostile 200 must clear the verdict and leave the prior ranged value
    // untouched; a subsequent 50 must establish and store successfully.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-ranged-decode-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ranged wire decode canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("ranged wire decode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("ranged wire decode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected hostile range refusal and valid range establishment (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn computed_range_wire_decoding_preserves_exact_endpoints_in_the_interpreter() {
    let scratch = unique_no_output_build_dir();
    for (name, fixture) in [
        (
            "field",
            fixture_roster::RUNTIME_WIRE_DECODE_RANGED_FIELD_EXIT,
        ),
        (
            "repeated",
            fixture_roster::RUNTIME_WIRE_DECODE_RANGED_REPEATED_EXIT,
        ),
    ] {
        let canary = pass_canary(fixture);
        let original = fs::read_to_string(canary.join("main.omg")).expect("ranged wire source");
        let source = if name == "field" {
            original
                .replace("[0..=100]", "[0..=(51 / 2) * 2]")
                .replace("self.buffer[2] = 50;", "self.buffer[2] = 51;")
                .replace("good.v == 50", "good.v == 51")
        } else {
            original.replace("[-2..=2]", "[0 - 1 / 2 * 4..=1 / 2 * 4]")
        };
        let project = scratch.join(name);
        fs::create_dir_all(&project).expect("computed range fixture directory");
        let main_path = project.join("main.omg");
        fs::write(&main_path, source).expect("computed range fixture source");
        // The shared checked compiler derives the standard-library and console
        // service bindings from this copied declaration and the source import.
        fs::copy(canary.join("build.omg"), project.join("build.omg"))
            .expect("ranged wire build declaration");
        let checked =
            compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
                .unwrap_or_else(|diagnostics| panic!("{name}: {diagnostics:#?}"));
        let outcome = interpret(&checked, &[]);
        assert_eq!(outcome.error, None, "{name}");
        assert_eq!(
            outcome.exit_code, 70,
            "{name}: rejected slots must remain intact and exact endpoints must decode"
        );
    }
    fs::remove_dir_all(scratch).expect("computed range fixture cleanup");
}

#[test]
fn runtime_wire_decode_ranged_repeated_exit_canary_runs() {
    // A hostile repeated element must clear the verdict without overwriting
    // its prior ranged slot; valid elements in the same payload still decode.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_RANGED_REPEATED_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-ranged-repeated-decode-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ranged repeated wire decode canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("ranged repeated wire decode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("ranged repeated wire decode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected repeated range refusal and valid establishment (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_rejects_noncanonical_bool_exit_canary_runs() {
    // Plain and repeated bool decodes must accept only canonical 0/1
    // representations, preserving prior values when a hostile 2 arrives.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_REJECTS_NONCANONICAL_BOOL_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-noncanonical-bool-decode-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("noncanonical bool wire decode canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("noncanonical bool wire decode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("noncanonical bool wire decode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected canonical bool enforcement (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_rejects_noncanonical_varint_exit_canary_runs() {
    // LEB128 values must use the fewest groups and may not carry bits beyond
    // u64, while the canonical ten-group u64 maximum remains valid.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_REJECTS_NONCANONICAL_VARINT_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-noncanonical-varint-decode-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("noncanonical varint wire decode canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("noncanonical varint wire decode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("noncanonical varint wire decode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected canonical LEB128 enforcement (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_rejects_scalar_width_overflow_exit_canary_runs() {
    // Wider hostile varints must not become valid i32/u32 values merely
    // because the final destination store truncates their high bits.
    let canary =
        pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_REJECTS_SCALAR_WIDTH_OVERFLOW_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-scalar-width-overflow-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("scalar width overflow wire decode canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("scalar width overflow wire decode canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("scalar width overflow wire decode canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected scalar carrier-width enforcement (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_nested_exit_canary_runs() {
    // Wire nested message fields: encode { header: { room_id: 300, kind: -2 },
    // depth: -64 } into [0x00, 0x00, 0x05, 0x00, 0xAC, 0x02, 0x01, 0x03,
    // 0x01, 0x7F] (hand-computed in the canary header -- the nested field is
    // tag + LENGTH varint + the child's fields with NO era discriminator),
    // then decode back into a fresh value: ok = true, read = 10, and every
    // field including the nested ones equals the original. Exits 70 on a
    // full match.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_NESTED_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wire-nested-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire nested roundtrip canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire nested roundtrip canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire nested roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 round trip to preserve the nested message (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_rejects_bad_nested_length_exit_canary_runs() {
    // Wire nested message fields, failure path: a hand-built buffer whose
    // nested LENGTH byte says 6 where the child's fields occupy 5 must fail
    // the decode -- the nested CLOSE check clears the sticky ok because the
    // cursor lands one byte before the declared end bound (walk in the
    // canary header). Exits 70 on the failure path.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_REJECTS_BAD_NESTED_LENGTH_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-nested-length-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire bad-nested-length canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire bad-nested-length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire bad-nested-length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the decoder to reject a nested length that disagrees with the content (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_roundtrip_repeated_exit_canary_runs() {
    // Wire FixedVec field: encode { sensor_id: 7, samples: [150, -2] len 2,
    // flag: true } into [0x00, 0x00, 0x07, 0x01, 0x03,
    // 0xAC, 0x02, 0x03, 0x02, 0x01] (hand-computed in the canary header --
    // the repeated field packs LENGTH-delimited: tag + byte-length varint +
    // the live element varints, no per-element tags), then decode back into
    // a fresh value: ok = true, read = 10, both live elements and intrinsic
    // length round-trip. Exits 70 on a full match.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ROUNDTRIP_REPEATED_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wire-repeated-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire repeated roundtrip canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire repeated roundtrip canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire repeated roundtrip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 repeated field to round-trip (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_rejects_repeated_overflow_exit_canary_runs() {
    // Wire repeated fields, failure paths: a packed payload carrying MORE
    // elements than the declared maximum must fail the decode (the unrolled
    // guarded reads stop at the maximum, so the cursor lands short of the
    // declared end bound and the CLOSE check clears ok -- the count
    // companion reports the capped element count), and a hostile byte-length
    // claiming more than the buffer holds must fail at the OPEN check
    // without reading out of bounds. Exits 70 when both decodes report
    // failure (walk in the canary header).
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_REJECTS_REPEATED_OVERFLOW_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-repeated-overflow-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire repeated overflow canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire repeated overflow canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire repeated overflow canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the decoder to reject repeated payloads past the declared maximum or the buffer (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_rejects_wrong_era_exit_canary_runs() {
    // Wire stage 2b: a hand-built buffer carrying era byte 5 (the schema's
    // current era is 0) must fail to decode -- the era discriminator is the
    // first expected byte and the failure flag is sticky. The canary exits 70
    // on the failure path (`ok` = false).
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_REJECTS_WRONG_ERA_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wire-wrong-era-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire wrong-era canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire wrong-era canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire wrong-era canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected decode to reject a non-current era discriminator (exit 70 on the failure path), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_encode_string_exit_canary_runs() {
    // Wire stage 2a, String fields: a String field rides as tag varint +
    // LENGTH varint (byte count) + raw UTF-8 bytes (no NUL, no padding), and
    // must encode LAST. The canary checks the seven expected bytes for
    // { count: 7, label: "hi" } (hand-computed in its header comment) and the
    // written count in-language; exits 70 when byte-exact.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_STRING_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-wire-string-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire encode string canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire encode string canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire encode string canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the compact_binary v0 encoder to frame the String field as len varint + raw bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_encode_byte_slice_exit_canary_runs() {
    // Wire stage 2 (#43), borrowed `&[u8]` fields: a fat-slice bytes field
    // constructed from a fixed-array subslice (`{ bytes: self.source[0..2] }`)
    // materializes a `{ptr, len}` descriptor, and `encode` frames it as RAW
    // bytes (length varint + the bytes) through the same text-bytes append a
    // String uses. The canary checks the five expected bytes + the written count
    // in-language; exits 70 when byte-exact.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_BYTE_SLICE_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-wire-byte-slice-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire encode byte-slice canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire encode byte-slice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire encode byte-slice canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8]` field construction + encode to frame raw bytes (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_encode_borrowed_scalar_slice_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_ENCODE_BORROWED_SCALAR_SLICE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-borrowed-scalar-slice-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire encode borrowed scalar-slice canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("wire encode borrowed scalar-slice canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pass scalar-slice encoder to emit exact packed zigzag varints (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let report = fs::read_to_string(scratch.join("04_wire_protocols.txt"))
        .expect("wire protocol report should retain encode obligations");
    for expected in [
        "encode requirement: Encode<compact_binary, Telemetry>",
        "encode requirement identity: 0x",
        "encode obligations:",
        "field 0: runtime element count; two scalar passes per element; remaining output capacity covers exact packed payload",
    ] {
        assert!(
            report.contains(expected),
            "wire report should contain `{expected}`\n{report}"
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decode_byte_slice_exit_canary_runs() {
    // Wire stage 2 (#43), borrowed `&[u8]` ZERO-COPY decode: `decode` reads
    // a byte-length varint and stores a fat `{ptr, len}` descriptor viewing the
    // buffer in place (the `ReadWireByteSlice` op). The canary round-trips and
    // RE-ENCODES the decoded value to prove the view is content-correct (ptr +
    // len point at the right buffer bytes); exits 70 when byte-exact.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODE_BYTE_SLICE_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-decode-byte-slice-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire decode byte-slice canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire decode byte-slice canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire decode byte-slice canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `&[u8]` zero-copy decode to recover a content-correct buffer view (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decoded_byte_slice_index_exit_canary_runs() {
    // Consuming a decoded zero-copy `&[u8]`: a runtime-indexed element read
    // (`data[i]`) in transition-ARGUMENT position must be materialized by reading
    // through the descriptor's data pointer. It used to fall through every
    // argument strategy and was never written (parameter kept uninitialized
    // bytes); now resolved as a value operand. Exits 70 when decoded.bytes[0]==72.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODED_BYTE_SLICE_INDEX_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-decoded-byte-slice-index-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire decoded byte-slice index canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire decoded byte-slice index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire decoded byte-slice index canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected indexing a decoded `&[u8]` under a length guard to read the right byte (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_wire_decoded_byte_slice_len_exit_canary_runs() {
    // `.len` of a `&[u8]` held as a struct FIELD must resolve to the descriptor's
    // runtime len slot. The place resolver used to drop a
    // `<struct>.<descriptor-field>.len` path (the `.len` step has no data layout),
    // so `let n = decoded.bytes.len` emitted no write and `n` held garbage. The
    // length is genuinely runtime (decoded from a varint), so a correct read
    // proves the len slot is targeted; exits 70 when n == 2.
    let canary = pass_canary(fixture_roster::RUNTIME_WIRE_DECODED_BYTE_SLICE_LEN_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-decoded-byte-slice-len-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire decoded byte-slice .len canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire decoded byte-slice .len canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire decoded byte-slice .len canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `.len` of a decoded `&[u8]` field to read the descriptor len slot (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

use super::*;

#[test]
fn runtime_method_view_write_after_last_use_exit_canary_runs() {
    // Lifetimes stage 1, NLL complement of
    // fail/borrow/method_view_receiver_unrelated_field_write: the
    // receiver-wide loan of a method-returned view ends at the view's LAST
    // USE, so a later write to another field of the same receiver compiles
    // and both writes land (7 + 63 = 70).
    let canary = pass_canary("borrow/runtime_method_view_write_after_last_use_exit");
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
    let canary = pass_canary("borrow/runtime_view_of_view_chain_exit");
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
    let canary = fail_canary("slices/termination_slice_length_order_unimplemented");
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
    let canary = pass_canary("termination/runtime_shrinking_slice_recursion_exit");
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
    let canary = pass_canary("wire/runtime_wire_encode_primitive_exit");
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
fn runtime_wire_encode_era_discriminator_exit_canary_runs() {
    // Wire stage 2a + frozen decision 10: a schema with one declared version
    // block snapshots it as era 0, so the CURRENT body encodes era 1 -- the
    // first byte of every encoded message. The canary asserts the era byte,
    // the recycled field's tag/value bytes, and the written count; exits 70
    // when byte-exact.
    let canary = pass_canary("wire/runtime_wire_encode_era_discriminator_exit");
    let scratch = std::env::temp_dir().join(format!("omega-wire-era-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("wire era discriminator canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("wire era discriminator canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("wire era discriminator canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the current body to encode era 1 after one version block (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn numbered_case_identities_compile() {
    let canary = pass_canary("wire/numbered_case_identities");
    let build_dir =
        std::env::temp_dir().join(format!("omega-numbered-cases-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = production_compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: false,
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
    let canary = pass_canary("wire/runtime_wire_roundtrip_primitive_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_ranged_field_exit");
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
fn runtime_wire_decode_ranged_repeated_exit_canary_runs() {
    // A hostile repeated element must clear the verdict without overwriting
    // its prior ranged slot; valid elements in the same payload still decode.
    let canary = pass_canary("wire/runtime_wire_decode_ranged_repeated_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_rejects_noncanonical_bool_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_rejects_noncanonical_varint_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_rejects_scalar_width_overflow_exit");
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
    let canary = pass_canary("wire/runtime_wire_roundtrip_nested_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_rejects_bad_nested_length_exit");
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
    let canary = pass_canary("wire/runtime_wire_roundtrip_repeated_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_rejects_repeated_overflow_exit");
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
    let canary = pass_canary("wire/runtime_wire_decode_rejects_wrong_era_exit");
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
    let canary = pass_canary("wire/runtime_wire_encode_string_exit");
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
    let canary = pass_canary("wire/runtime_wire_encode_byte_slice_exit");
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
    let canary = pass_canary("wire/runtime_wire_encode_borrowed_scalar_slice_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-wire-borrowed-scalar-slice-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host_with_auxiliary_artifacts(&canary, scratch.clone())
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
    let canary = pass_canary("wire/runtime_wire_decode_byte_slice_exit");
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
    let canary = pass_canary("wire/runtime_wire_decoded_byte_slice_index_exit");
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
    let canary = pass_canary("wire/runtime_wire_decoded_byte_slice_len_exit");
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

#[test]
fn runtime_call_result_binary_operand_exit_canary_runs() {
    // A state-call result used as an operand of a larger value (`x = f() + 1`,
    // `x = max(y, f()+1)`) must apply the operator, not collapse to just the call's
    // result. The dispatch-body mutation path had a statement-level "copy call result
    // to target" shortcut that fired even when the call was a sub-expression, dropping
    // the `+1`/`max`. It now fires only for a bare, non-builtin call value.
    let canary = pass_canary("expressions/runtime_call_result_binary_operand_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-call-result-binary-operand-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("call-result-binary-operand canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("call-result-binary-operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("call-result-binary-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a call result used as a binary/max operand to apply the operator (exit 70), \
         got {:?} (71 = the surrounding operator was dropped and only the call result written)\
         \nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cast_operand_exit_canary_runs() {
    // A numeric `as` cast used as a binary operand (`self.a + (self.b as f64)`) must
    // convert the source in place via a Convert value operand, not be dropped. Covers
    // int->float, float->int, and integer widening; exits 70 only when all convert.
    let canary = pass_canary("expressions/runtime_cast_operand_exit");
    let scratch = std::env::temp_dir().join(format!("omega-cast-operand-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cast-operand canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("cast-operand canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `a + (b as T)` casts in operand position to convert correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_f32_arithmetic_exit_canary_runs() {
    // Single-precision f32 store/copy/compare and field arithmetic must use the
    // single-precision SSE forms (movd + ucomiss/addss/...) keyed on byte_size 4,
    // with is_float recognizing F32 -- previously f32 was compared/operated as an
    // integer or as double precision. Exits 70 only when every f32 op is correct.
    let canary = pass_canary("expressions/runtime_f32_arithmetic_exit");
    let scratch = std::env::temp_dir().join(format!("omega-f32-arith-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 arithmetic canary should compile");

    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 store/copy/compare + add/sub/mul/div to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_f32_local_arithmetic_exit_canary_runs() {
    // Single-precision f32 arithmetic/comparison into LOCAL variables (frame slots),
    // the companion to the field-based runtime_f32_arithmetic_exit. A local f32 binary
    // write reaches the pre-resolved-place selection path, which previously did no f32
    // narrowing -- so `let c: f32 = a + b` ran addss over an f64 bit pattern (garbage).
    // Also covers a cast of a folded f32 arithmetic expression into a local int
    // (`let n: i32 = c as i32`). Exits 70 only when add/sub/mul/div, an f32 `<`
    // compare, and the f32->i32 cast all evaluate correctly.
    let canary = pass_canary("expressions/runtime_f32_local_arithmetic_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-f32-local-arith-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("f32 local arithmetic canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("f32 local arithmetic canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32 local add/sub/mul/div + compare + cast to evaluate correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_multi_arm_value_transition_exit_canary_runs() {
    // A value-returning machine whose body is a 3-arm guarded transition must select
    // the MIDDLE arm, not fall through to the default. The guard-failure jump used to
    // land on the matched arm's own body copy (before its forward skip), so a failed
    // first-arm guard skipped the middle arm. Exits 70 only when all three arms
    // (first/middle/default) select correctly.
    let canary = pass_canary("calls/runtime_multi_arm_value_transition_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-multi-arm-value-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("multi-arm value transition canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("multi-arm value transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multi-arm value transition canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a 3-arm value transition to select first/middle/default correctly (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_transition_unsigned_guard_exit_canary_runs() {
    // A value-transition arm guard on an UNSIGNED (u32) operand must branch with
    // unsigned comparison conditions. The leaf value-transition guard path picked
    // the SIGNED jcc regardless of operand signedness (only the dispatch-edge path
    // post-processed the operator with the operand's unsignedness). For a u32 with
    // its top bit set (4000000000 > INT_MAX), `x <= 2` is FALSE unsigned (correct)
    // but TRUE signed (wrong). A signed mis-compare selects the first arm and exits
    // 71; a correct unsigned compare selects the default arm and exits 70.
    let canary = pass_canary("calls/runtime_value_transition_unsigned_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-transition-unsigned-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned value-transition guard canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned value-transition guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned value-transition guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a u32 value-transition guard to branch unsigned (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_const_array_length_exit_canary_runs() {
    // COMPTIME STAGE 1: `slots: [i64; table_size()]` sizes a data field by an
    // build-time-admissible machine call, evaluated by the reference interpreter
    // before checking/layout (the callee computes 12 + 4, pinning evaluation,
    // not literal forwarding). Indexing slots[15] only type-checks if the
    // substituted Literal(16) reached the range checker, and the values only
    // read back if layout sized the field as 16 elements -- identically to a
    // written `[i64; 16]`. Exits 70 only when both ends hold their values.
    let canary = pass_canary("comptime/runtime_const_array_length_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-const-array-length-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("const array length canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("const array length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const array length canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `[i64; table_size()]` to const-evaluate to 16 and behave exactly like a literal length (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_fixed_vec_round_trip_exit_canary_runs() {
    // ALLOCATOR STORY STAGE 1: the fixed-capacity vec pattern (core
    // FixedVec<T, const N: usize>, hand-instantiated at i32/N=4 pending
    // generic machine instantiation) round-trips at runtime with every
    // bounds obligation PROOF-discharged through contract chaining: clear
    // establishes room, push consumes it and guarantees non-emptiness plus
    // the popped slot's bound, pop/get consume push's guarantees. The guard
    // ladder checks the actual data flow (pushed value lands, pop returns it
    // and shrinks, a second clear/push cycle overwrites slot 0, final length
    // is 1) and exits 70 only when all hold.
    let canary = pass_canary("collections/runtime_fixed_vec_round_trip_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-fixed-vec-round-trip-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("fixed vec round trip canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("fixed vec round trip canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("fixed vec round trip canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the proof-discharged push/pop/get round trip to hold its values (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_negative_ops_exit_canary_runs() {
    // Float operations with negatives -- comparisons (the ucomisd unsigned-flags case),
    // a negative float->int cast (truncation toward zero), and a negative multiply.
    // Exits 70.
    let canary = pass_canary("arithmetic/runtime_float_negative_ops_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-float-negative-ops-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float negative ops canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float negative ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float compares/cast/multiply with negatives to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float32_array_conversion_exit_canary_runs() {
    // f32 ARRAY (the separate f32 codegen path: mulss/addss/ucomiss) plus an int<->f64 round-trip
    // cast. f32 sum 1.5+2.5+2.25 = 6.25, then 7 as f64 * 2.0 truncated to i32 = 14. Exits 70.
    let canary = pass_canary("arithmetic/runtime_float32_array_conversion_exit");
    let scratch = std::env::temp_dir().join(format!("omega-float32-array-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("f32 array + conversion canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("f32 array + conversion canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected f32-array sum 6.25 + int<->f64 round-trip == 14 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_value_call_let_combine_exit_canary_runs() {
    // The SAFE side of the shared-result-slot boundary: a computing value-machine called twice in
    // one state whose results are bound to locals and combined in one expression is materialized
    // eagerly and correct (dbl(3)=6, dbl(4)=8 -> 6*100+8 == 608 -> exit 70). Guards against
    // re-broadening the shared-value-call-slot fence into a false positive on this valid pattern.
    let canary = pass_canary("calls/runtime_value_call_let_combine_exit");
    let scratch = std::env::temp_dir().join(format!("omega-vc-letcombine-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-call let-combine canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("value-call let-combine canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("value-call let-combine canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two value-calls bound to locals and combined in one expression to keep distinct \
         results (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_float_nan_comparison_exit_canary_runs() {
    // NaN comparison honors IEEE in native codegen: `!=` is true, the other five operators
    // are false. Guards on `ucomis*` must test the parity flag (a `jp` branch) or 4 of 6 take
    // the wrong arm. The canary checks all six against a NaN operand and exits 70 iff correct.
    let canary = pass_canary("arithmetic/runtime_float_nan_comparison_exit");
    let scratch = std::env::temp_dir().join(format!("omega-float-nan-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("float NaN comparison canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("float NaN comparison canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected NaN comparisons to follow IEEE (only != true; exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_saturating_domain_exit_canary_runs() {
    // Saturating arithmetic clamps at the type bounds (the core arithmetic-safety domain): u8
    // add/sub/mul over- and under-flow -> 255/0/255, i8 signed positive overflow -> 127, i8 signed
    // negative underflow -> -128 (checked as +128 == 0). All self-checked -> exit 70.
    let canary = pass_canary("arithmetic/runtime_saturating_domain_exit");
    let scratch = std::env::temp_dir().join(format!("omega-saturating-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("saturating domain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("saturating domain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("saturating domain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Saturating arithmetic to clamp at type bounds in all four directions (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_i64_signed_arithmetic_exit_canary_runs() {
    // i64 signed arithmetic beyond i32: multiply past 2^32, signed div/mod with a negative dividend
    // (sign follows dividend), and a 64-bit shift (1<<40). All chained -> exit 70.
    let canary = pass_canary("arithmetic/runtime_i64_signed_arithmetic_exit");
    let scratch = std::env::temp_dir().join(format!("omega-i64-signed-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("i64 signed arithmetic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("i64 signed arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("i64 signed arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected i64 multiply/div/mod/shift at scale to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_cast_sign_zero_extension_exit_canary_runs() {
    // Width conversions pick the right extension (movsx vs movzx): -1 as i8 as i32 == -1
    // (sign-extend), -1 as u8 as i32 == 255 (zero-extend), 200 as i8 as i32 == -56 (truncate +
    // sign-extend). All chained -> exit 70.
    let canary = pass_canary("arithmetic/runtime_cast_sign_zero_extension_exit");
    let scratch = std::env::temp_dir().join(format!("omega-cast-sign-zero-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("cast sign/zero extension canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("cast sign/zero extension canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cast sign/zero extension canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sign/zero extension + truncation casts correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bitwise_high_ops_exit_canary_runs() {
    // Bitwise ops on u32 above i32::MAX: a=0xF0F0F0F0, b=0x0F0F0F0F. XOR/AND/OR, a shift+mask
    // nibble extract, and NOT-via-XOR all chained -> exit 70.
    let canary = pass_canary("arithmetic/runtime_bitwise_high_ops_exit");
    let scratch = std::env::temp_dir().join(format!("omega-bitwise-high-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bitwise high ops canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bitwise high ops canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bitwise high ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected u32 bitwise ops at high values to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_unsigned_high_comparison_exit_canary_runs() {
    // Unsigned comparison above i32::MAX (a = u32::MAX, b = 1): a signed setcc would invert every
    // ordered result. All six operators chained give the unsigned answer -> exit 70.
    let canary = pass_canary("arithmetic/runtime_unsigned_high_comparison_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-unsigned-high-cmp-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("unsigned high comparison canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("unsigned high comparison canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("unsigned high comparison canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected unsigned comparisons of u32::MAX vs 1 to be correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_signed_modulo_shift_edges_exit_canary_runs() {
    // Sign-sensitive integer codegen: truncated signed modulo with negatives (-7%3==-1, 7%-3==1),
    // arithmetic vs logical right shift (-16>>2==-4 SAR, 16u32>>2==4 SHR), and a runtime shift
    // amount (1<<5==32). Exits 70.
    let canary = pass_canary("arithmetic/runtime_signed_modulo_shift_edges_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-signed-mod-shift-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("signed modulo/shift edges canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signed modulo/shift canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("signed modulo/shift edges canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected signed modulo + arithmetic/logical/runtime shifts correct (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_newton_sqrt_exit_canary_runs() {
    // Newton's method for a square root (an iterative numerical algorithm): x <- (x + S/x)/2
    // over f64, six iterations from 1.0 on S=2.0 -> sqrt(2) ~= 1.41421; checks
    // 1.414 < x < 1.415 -> exit 70.
    let canary = pass_canary("arithmetic/runtime_newton_sqrt_exit");
    let scratch = std::env::temp_dir().join(format!("omega-newton-sqrt-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("newton sqrt canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("newton sqrt canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Newton's method to converge to sqrt(2) in (1.414, 1.415) (exit 70); got {:?} -- a float div/compare regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_monte_carlo_pi_exit_canary_runs() {
    // Monte Carlo pi estimation driven by the xorshift32 PRNG: 64 random points, count
    // those inside the quarter circle (px*px+py*py < 100*100). Deterministic from seed 1:
    // 53 inside, scaled estimate 400*53/64 = 331 (pi ~= 3.31) -> exit 70.
    let canary = pass_canary("arithmetic/runtime_monte_carlo_pi_exit");
    let scratch = std::env::temp_dir().join(format!("omega-monte-carlo-pi-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("monte carlo pi canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("monte carlo pi canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected Monte Carlo pi (seed 1, 64 points) to count 53 inside / estimate 331 (exit 70); got {:?} -- the count on regression\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_gcd_euclid_exit_canary_runs() {
    // The iterative Euclidean GCD: `(a,b) = (b, a%b)` until b==0. gcd(1071,462)=21.
    // A two-variable loop with a runtime modulo; self-checks the result -> exit 70.
    let canary = pass_canary("arithmetic/runtime_gcd_euclid_exit");
    let scratch = std::env::temp_dir().join(format!("omega-gcd-euclid-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("gcd euclid canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("gcd euclid canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("gcd euclid canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Euclidean GCD to reduce 1071,462 to 21 (exit 70); got {:?} (a non-70 code is the wrong gcd)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_rpn_evaluator_exit_canary_runs() {
    // A reverse-Polish stack evaluator (a stack VM): push numbers, pop-pop-op-push for
    // operators, over a token array. Evaluates `3 4 + 5 *` to 35 -> exit 70.
    let canary = pass_canary("collections/runtime_rpn_evaluator_exit");
    let scratch = std::env::temp_dir().join(format!("omega-rpn-eval-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("rpn evaluator canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("rpn evaluator canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("rpn evaluator canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the RPN stack VM to evaluate 3 4 + 5 * to 35 (exit 70); got {:?} (a non-70 code is the wrong result)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_activity_selection_greedy_exit_canary_runs() {
    // Greedy activity selection: given activities sorted by finish, take each that starts
    // no earlier than the last chosen finish. Six activities yield 3 non-overlapping ->
    // exit 70.
    let canary = pass_canary("collections/runtime_activity_selection_greedy_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-activity-greedy-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("activity selection canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("activity selection canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("activity selection canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected greedy activity selection to pick 3 non-overlapping (exit 70); got {:?} (the count on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_maze_pathfind_exit_canary_runs() {
    // Shortest-path BFS on a 5x5 grid maze (implicit grid neighbours + walls, distinct from
    // the adjacency-matrix BFS). The shortest distance from cell 0 to cell 24 through the
    // snaking corridor is 16 -> exit 70.
    let canary = pass_canary("collections/runtime_maze_pathfind_exit");
    let scratch = std::env::temp_dir().join(format!("omega-maze-pathfind-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("maze pathfind canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("maze pathfind canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("maze pathfind canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected grid-BFS shortest distance 0->24 to be 16 (exit 70); got {:?} (the distance on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

// UN-IGNORED 2026-07-10g: the latent hang this exposed (infinite backtracking,
// parked 2026-07-06 as "tracked separately" -- an audit found NOTHING tracked
// it) no longer reproduces: native exits 70 promptly and the interpreter
// agrees. One of the intervening arcs (the indexed family completion, the
// domain/witness work, or the cross-callee fixes) repaired the underlying
// pattern; this canary now guards the whole try/prune/undo shape end to end.
#[test]
fn runtime_nqueens_backtracking_exit_canary_runs() {
    // N-queens count by backtracking (try/prune/undo): cols[r] is the column tried for row
    // r and doubles as the state stack; conflicts are column or diagonal. N=4 has exactly 2
    // solutions -> exit 70 (a discriminating count).
    let canary = pass_canary("collections/runtime_nqueens_backtracking_exit");
    let scratch = std::env::temp_dir().join(format!("omega-nqueens-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nqueens backtracking canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("nqueens backtracking canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected N=4 queens to have exactly 2 solutions (exit 70); got {:?} (the count on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_coin_change_dp_exit_canary_runs() {
    // Coin-change minimisation by dynamic programming: dp[a] = fewest coins for amount a,
    // relaxing dp[a] toward 1 + dp[a-c] over a computed subproblem index. Coins {1,3,4},
    // amount 6 -> 2 coins (3+3) -> exit 70.
    let canary = pass_canary("collections/runtime_coin_change_dp_exit");
    let scratch = std::env::temp_dir().join(format!("omega-coin-change-dp-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("coin change dp canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("coin change dp canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("coin change dp canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected DP min coins for 6 with {{1,3,4}} to be 2 (exit 70); got {:?} (dp[6] on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bfs_traversal_exit_canary_runs() {
    // Breadth-first search over a 4-node graph (adjacency matrix + FIFO queue + visited
    // set): from node 0 the frontier expands level by level, visit order 0,1,2,3, all four
    // reached -> exit 70.
    let canary = pass_canary("collections/runtime_bfs_traversal_exit");
    let scratch = std::env::temp_dir().join(format!("omega-bfs-traversal-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bfs traversal canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bfs traversal canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bfs traversal canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected BFS to visit 0,1,2,3 in order and reach all 4 nodes (exit 70); got {:?} (the visit count on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_hash_table_exit_canary_runs() {
    // An open-addressing hash table with linear probing (the associative map): parallel
    // keys/vals/used arrays, hash k%8, probe forward with wrap past occupied slots, look
    // back up. Keys 6,14,7,15 collide and force a wrap; their values sum to 246 -> exit 70.
    let canary = pass_canary("collections/runtime_hash_table_exit");
    let scratch = std::env::temp_dir().join(format!("omega-hash-table-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("hash table canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("hash table canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("hash table canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the hash table (probe + wrap) to sum looked-up values to 246 (exit 70); got {:?} (the sum on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_matrix_multiply_exit_canary_runs() {
    // 2x2 matrix multiply (row-major flat storage, triple i/j/k loop, inner-product
    // accumulation with computed flat indices). [[1,2],[3,4]] * [[5,6],[7,8]] =
    // [[19,22],[43,50]] -> exit 70.
    let canary = pass_canary("collections/runtime_matrix_multiply_exit");
    let scratch = std::env::temp_dir().join(format!("omega-matrix-mul-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("matrix multiply canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("matrix multiply canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("matrix multiply canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 2x2 matmul to yield [[19,22],[43,50]] (exit 70); got {:?} (a non-70 code is the wrong C[0])\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_ring_buffer_queue_exit_canary_runs() {
    // A FIFO ring-buffer queue: a fixed [i32;4] with head/tail advancing modulo the
    // capacity (explicit wrap) and a count guard. Interleaved enqueue/dequeue forces both
    // pointers to wrap; each dequeue is checked against a running counter so FIFO order is
    // pinned. All of 1..6 dequeued in order -> exit 70.
    let canary = pass_canary("collections/runtime_ring_buffer_queue_exit");
    let scratch = std::env::temp_dir().join(format!("omega-ring-buffer-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("ring buffer queue canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("ring buffer queue canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("ring buffer queue canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the ring buffer to preserve FIFO order 1..6 (exit 70); got {:?} (a non-70 code is where order broke)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_bubble_sort_exit_canary_runs() {
    // Bubble sort with nested loops, the adjacent index `j+1` via a field, a field-bound
    // compare, and a value-swap. Sorts [5,2,8,1,9,3] and self-checks four cells -> 70.
    let canary = pass_canary("collections/runtime_bubble_sort_exit");
    let scratch = std::env::temp_dir().join(format!("omega-bubble-sort-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bubble sort canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bubble sort canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bubble sort canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected bubble sort to order the array (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_2d_transpose_exit_canary_runs() {
    // A 2D matrix transpose over a flat array via the linear-counter sidestep: the
    // (row,col) and transposed output index are computed into fields, then used as plain
    // indices. Self-checks four transposed cells -> exit 70. Proves 2D/matrix data.
    let canary = pass_canary("collections/runtime_2d_transpose_exit");
    let scratch = std::env::temp_dir().join(format!("omega-2d-transpose-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("2d transpose canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("2d transpose canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("2d transpose canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the 2D transpose to place cells correctly (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_through_guard_chain_exit_canary_runs() {
    // An index bound carried across a CHAIN of convergent-arm guards (`d<0 {true->t
    // _->t}`) that neither name nor rewrite x. Before convergent arms were treated as
    // a single unconditional predecessor, each guard split dropped the bound. Compiling
    // + reading arr[3]=70 -> exit 70 confirms the bound survives the chain.
    let canary = pass_canary("collections/runtime_indexed_through_guard_chain_exit");
    let scratch = std::env::temp_dir().join(format!("omega-guard-chain-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed-through-guard-chain canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-through-guard-chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-through-guard-chain canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the index bound to survive the convergent-guard chain (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_binary_search_exit_canary_runs() {
    // Binary search for 50 in a sorted 7-element array narrows in BOTH directions
    // (lo=mid+1 then hi=mid-1) and must find it at exactly index 4. Locks the computed
    // midpoint, the indexed read into a field, and both pointer updates. Exits 70.
    let canary = pass_canary("collections/runtime_binary_search_exit");
    let scratch = std::env::temp_dir().join(format!("omega-binary-search-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("binary search canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("binary search canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("binary search canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected binary search to find 50 at index 4 (exit 70); got {:?} (71=wrong index, 72=not found)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_two_pointer_palindrome_exit_canary_runs() {
    // A two-pointer palindrome check whose DECREASING pointer `j` stays >= 0 only
    // because j > i >= 0 -- proven by chaining the loop ordering `i < j` with `i`'s
    // non-negativity (non_negative_is_proven_via_ordering). Compiling + exiting 70
    // confirms the decreasing-counter lower bound is derived and the walk is correct.
    let canary = pass_canary("collections/runtime_two_pointer_palindrome_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-two-pointer-palindrome-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("two-pointer palindrome canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("two-pointer palindrome canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("two-pointer palindrome canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pointer palindrome walk to confirm [3,7,9,7,3] (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_struct_array_field_exit_canary_runs() {
    // Nested data: a struct field that is an array of structs (`self.g.pts[k].x`), const-indexed,
    // sub-fields read as binary operands. Sum = 20+30+18+2 = 70.
    let canary = pass_canary("collections/runtime_nested_struct_array_field_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-struct-array-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested struct-array field canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested struct-array field canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested struct-array field canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected struct->array-of-structs->field sum 20+30+18+2 == 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_enum_grid_scan_exit_canary_runs() {
    // Scan an array of enums (tile grid) by runtime index via the bind-to-local workaround: read
    // grid[i] into self.c, then match. grid=[Wall,Door,Floor,Door,Wall] -> 2 Doors -> exit 70.
    let canary = pass_canary("collections/runtime_enum_grid_scan_exit");
    let scratch = std::env::temp_dir().join(format!("omega-enum-grid-scan-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("enum grid scan canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("enum grid scan canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("enum grid scan canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected enum-grid scan to count 2 Doors (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_two_indexed_reads_binary_exit_canary_runs() {
    // Two runtime-indexed reads at DISTINCT indices as operands of one binary: s = nums[i] + nums[j].
    // nums=[30,99,40], i=0, j=2 -> 30+40 = 70 (the 99 decoy catches a dropped index).
    let canary = pass_canary("collections/runtime_two_indexed_reads_binary_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-two-indexed-reads-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("two-indexed-reads binary canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("two-indexed-reads binary canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("two-indexed-reads binary canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected nums[0]+nums[2] = 30+40 = 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_struct_field_temp_arith_exit_canary_runs() {
    // The sound workaround for arithmetic on a runtime-indexed array-of-structs field: read the
    // field into a scalar `self.t` first, then compute. arr[1]={30,40}; t1+t2 = 70. (A direct
    // `arr[i].x + 5` is refused -- no machine-indexed struct-field value operand yet.)
    let canary = pass_canary("collections/runtime_struct_field_temp_arith_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-struct-field-temp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("struct-field-temp arithmetic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("struct-field-temp arithmetic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("struct-field-temp arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected field-temp read of arr[1].x + arr[1].y == 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_struct_write_loop_exit_canary_runs() {
    // A whole-struct write to a runtime-indexed array-of-structs element in a loop (entity-array
    // population): `self.arr[self.i] = Pt{..}`. Fill 3 elements, sum 10+15+10 = 35 -> exit 70.
    let canary = pass_canary("collections/runtime_indexed_struct_write_loop_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-indexed-struct-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed struct-write loop canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed struct-write loop canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed struct-write loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime-indexed whole-struct writes summing to 35 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn std_option_runtime_match_exit_canary_runs() {
    // The std `Optional<T>` works at runtime for presence/absence and payload
    // extraction. `b` is never written, so its all-zero home representation
    // must dispatch as None.
    let canary = pass_canary("collections/std_option_runtime_match_exit");
    let scratch = std::env::temp_dir().join(format!("omega-std-option-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("std Optional runtime match canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("std Optional runtime match canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("std option runtime match canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected std Optional Some/None construct + match to exit 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_indexed_read_then_guard_exit_canary_runs() {
    // The sound pattern for guarding a runtime-indexed array element: read it into a place first,
    // then compare that place (a direct `transition nums[i] > 5` silently takes the first arm).
    // nums[2]=9 -> v=9 -> 9>5 true -> exit 70; a dropped index would read nums[0]=1 -> 71.
    let canary = pass_canary("collections/runtime_indexed_read_then_guard_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-indexed-read-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("indexed-read-then-guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-read-then-guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-read-then-guard canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime-indexed read into a place then guard (nums[2]=9>5) to exit 70; got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_row_const_column_write_exit_canary_runs() {
    // The working side of the 2D-write boundary: a runtime ROW index with a CONST column
    // (`grid[r][0]`, `grid[r][1]`) lowers correctly. Fill both columns of both rows by runtime row,
    // sum 10+15+20+25 = 70. (The runtime-COLUMN case is rejected; see the fail canary.)
    let canary = pass_canary("collections/runtime_row_const_column_write_exit");
    let scratch = std::env::temp_dir().join(format!("omega-row-const-col-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-row const-column write canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("runtime-row const-column write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("runtime-row const-column write canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected runtime-row const-column 2D writes to sum 10+15+20+25 == 70 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_nested_array_const_index_exit_canary_runs() {
    // A 2D array [[i32;2];2]: const-indexed reads and writes work. Fill all four cells, sum =
    // 1+2+3+4 = 10 -> exit 70. (Runtime-column 2D indexing is a separate known gap.)
    let canary = pass_canary("collections/runtime_nested_array_const_index_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-nested-array-const-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("nested array const-index canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested array const-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested array const-index canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected 2D-array const-index sum 1+2+3+4 == 10 (exit 70); got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_whole_array_value_copy_exit_canary_runs() {
    // Whole-array value copy: `self.b = self.a` copies contents, so mutating self.b[0] leaves
    // self.a untouched. Discriminates both ways: a keeps (5,6,7), b becomes (99,6,7) -> exit 70.
    let canary = pass_canary("collections/runtime_whole_array_value_copy_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-whole-array-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("whole-array value copy canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("whole-array value copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("whole-array value copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected whole-array copy to be independent (a unchanged, exit 70); got {:?} (aliased source value on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_whole_struct_value_copy_exit_canary_runs() {
    // Whole-STRUCT value copy: `self.p1 = self.p2` copies every field, so mutating
    // self.p2.x after the copy leaves self.p1 untouched (value, not alias). The
    // record complement of runtime_whole_array_value_copy_exit: p1 stays {30, 40}
    // even after p2.x = 99 -> exit 70.
    let canary = pass_canary("collections/runtime_whole_struct_value_copy_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-whole-struct-copy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("whole-struct value copy canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("whole-struct value copy canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("whole-struct value copy canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected whole-struct copy to be independent (p1 unchanged, exit 70); got {:?} (aliased source on regression)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_rule90_automaton_exit_canary_runs() {
    // A self-checking Rule 90 cellular automaton (the engine behind
    // samples/cellular_automaton): a sliding 3-cell window, the value-position rule
    // shift `(90 >> window) & 1`, plain-index array reads/writes, and a field-temp
    // double buffer. The live-cell counts of the first four generations (1,2,2,4) sum
    // to 9, so it exits 70 only when the computation is exactly right.
    let canary = pass_canary("collections/runtime_rule90_automaton_exit");
    let scratch =
        std::env::temp_dir().join(format!("omega-rule90-automaton-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("rule90 automaton canary should compile");
    let output = Command::new(scratch.join(executable_name()))
        .output()
        .expect("rule90 automaton canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the Rule 90 automaton's first-four-generation live-cell sum to be 9 (exit 70); got {:?} (a non-70 code is the actual sum)\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_fixed_array_field_guard_exit_canary_runs() {
    // Reading `self.cells[i].value` (fixed-array element field, constant index) in a
    // GUARD must apply the index: the guard-operand layout consumed the root field
    // without folding its out-of-band constant index, so `cells[1].value` read
    // element 0. The canary writes two distinct elements and guards each; a dropped
    // index exits 71 instead of 70.
    let canary = pass_canary("expressions/runtime_fixed_array_field_guard_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-fixed-array-field-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("fixed-array field guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("fixed-array field guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("fixed-array field guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `self.cells[i].value` guards to apply the constant index (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn runtime_fixed_array_field_value_exit_canary_runs() {
    // Reading `self.cells[2].value` (fixed-array element field, NON-ZERO constant
    // index) as a VALUE must apply the index. The GUARD path was fixed in 8e775fbd,
    // but the non-guard place resolvers used for value reads dropped the constant
    // index, so every `arr[const].field` value read aliased element 0. The canary
    // writes three distinct elements, reads the middle-high one into a field, and
    // guards it; a dropped index exits 71 instead of 70.
    let canary = pass_canary("expressions/runtime_fixed_array_field_value_exit");
    let scratch = std::env::temp_dir().join(format!(
        "omega-fixed-array-field-value-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("fixed-array field value canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("fixed-array field value canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("fixed-array field value canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `let d = self.cells[i].value` to apply the constant index (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn fixed_array_element_guard_canary_runs() {
    // A guard comparing a fixed-array element to a constant (`self.cells[2] == 7.0`,
    // cells `[f64; 4]`) must resolve one 8-byte element, not the whole 32-byte array
    // (which the encoder rejected). Promoted from pending once the guard-operand
    // layout applied the constant index; exits 0 when the guard reads cells[2].
    let canary = pass_canary("control_flow/fixed_array_element_guard");
    let scratch = std::env::temp_dir().join(format!(
        "omega-fixed-array-elem-guard-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("fixed-array element guard canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("fixed-array element guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("fixed-array element guard canary should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected `self.cells[2] == 7.0` to resolve one element and match (exit 0), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&scratch);
}

use super::assert_native_exit_code;
use super::fixture_roster;
use crate::{
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, fs, interpret,
    pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_duration_constructors_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_DURATION_CONSTRUCTORS_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-duration-ctors-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("duration constructors canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "duration constructors canary",
        "the seconds and milliseconds constructors should deliver their exact fields",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// Slice-descriptor element read/write/RMW against MACHINE storage -- the
// fixed-index read was width-0 (silently dropped) on x86_64. -> exit 1.

#[test]
fn runtime_slice_element_machine_roundtrip_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ELEMENT_MACHINE_ROUNDTRIP_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-slice-elem-rt-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("slice element machine roundtrip canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "slice-element machine-roundtrip canary",
        "machine-storage slice element reads, writes, and updates should roundtrip",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// RUNTIME-index slice-descriptor element read into a machine field. -> 1.

#[test]
fn runtime_slice_element_runtime_index_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_ELEMENT_RUNTIME_INDEX_READ_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-slice-elem-ri-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("slice element runtime index read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "slice-element runtime-index read canary",
        "the runtime-indexed slice element should reach machine storage",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A MEMBER-expression value-call arg (`self.grab(self.bx.inner)`) with a
// NESTED member read in the callee (`b.value`): the alias substitution's
// suffix must survive the member-rooted receiver (the suffix-drop class).
// pad=99/value=42 discriminate offset-0 reads. -> exit 1.

#[test]
fn runtime_member_arg_nested_read_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MEMBER_ARG_NESTED_READ_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-member-arg-nested-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("member arg nested read canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "member-argument nested-read canary",
        "the member-rooted alias should preserve its nested field suffix",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A call through a NESTED receiver field whose argument is an exact
// narrowing cast of a payload-bounded value
// (`self.scanner.append_byte(value as u8)`, the product lexer's `retain`
// state). The cast is a call-statement argument; validation now retains its
// exact-cast evidence there, so a host-target checked compile selects the
// attached entry and the interpreter reads the byte back. Native production
// is not claimed: the entry's sum-literal state argument is a separate Unit
// slice. -> 70.

#[test]
fn runtime_nested_receiver_cast_argument_exit_canary_interprets_and_establishes_its_entry() {
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_RECEIVER_CAST_ARGUMENT_EXIT);
    let main_path = canary.join("main.omg");
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(&main_path, None))
        .expect("nested receiver cast argument canary should compile to checked trees");
    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter oracle should read the cast byte back from the nested receiver, got {} ({:?})",
        outcome.exit_code, outcome.error
    );

    let established = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &main_path,
        Some(crate::native_hosted_target()),
    ))
    .expect("the host target selects the attached entry over the cast-argument call");
    assert!(
        established.selected_program_entry_machine().is_some(),
        "ProgramEntry establishment rejoins the receiver's attachment identity"
    );
}

// Type-scoped constructor with COMPUTED struct-literal fields delivers
// natively (the member-suffix drop in append_place_suffix is fixed). -> 1.

#[test]
fn runtime_constructor_computed_field_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONSTRUCTOR_COMPUTED_FIELD_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-ctor-computed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("constructor computed field canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "constructor computed-field canary",
        "the type-scoped constructor should deliver each computed field without root clobber",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// MACHINE-FIELD-bounded subslice local (`self.arr[self.lo..self.hi]`) --
// the START's indexed-address op is region-tagged now. len 3 -> exit 3.

#[test]
fn runtime_machine_bounded_subslice_local_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_MACHINE_BOUNDED_SUBSLICE_LOCAL_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-mach-subslice-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("machine-bounded subslice canary should compile");
    assert_native_exit_code(
        &compilation,
        3,
        "machine-bounded subslice canary",
        "the machine-field bounds should produce a subslice of length three",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Runtime-START subslice POINTER correctness (the ptr write was width-0 and
// silently dropped on x86_64; only len checks passed). s[0]==arr[1] -> 1.

#[test]
fn runtime_subslice_start_pointer_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_START_POINTER_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-subslice-ptr-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("subslice start pointer canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "subslice start-pointer canary",
        "the subslice pointer should begin at the runtime start index",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// LOOP-CARRIED accumulator through the dispatch self-loop: transition args
// stage through scratch so `acc + n` reads the pre-decrement n. -> 15 -> 1.

#[test]
fn runtime_loop_accumulator_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOOP_ACCUMULATOR_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-rec-accum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("recursive accumulator canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "loop-accumulator canary",
        "parallel assignment should preserve the pre-decrement accumulator input",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// 3-arg ROTATION through a self-transition loop -- the full parallel-assignment
// cycle (`-> rot(k-1, b, c, a)`). rot(3,1,2,3) -> a==1 -> exit 1.

#[test]
fn runtime_loop_rotation_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_LOOP_ROTATION_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-rec-rot-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("recursive rotation canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "loop-rotation canary",
        "parallel assignment should preserve the three-argument rotation cycle",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// `-> usize` written AFTER the machine clauses (`terminates -> usize`): the
// parser used to silently DROP it (skip-any-token fallback), so the machine
// parsed as VOID and callers bound ZII 0. Non-recursive value flows -> exit 1.

#[test]
fn runtime_post_clauses_return_type_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_POST_CLAUSES_RETURN_TYPE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-post-clauses-ret-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("post-clauses return type canary should compile");
    assert_native_exit_code(
        &compilation,
        1,
        "post-clauses return-type canary",
        "the post-clause return type should retain and deliver the value result",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A runtime slice `.len` read into a LOCAL binding in a VALUE position (`let n =
// s.len`), NOT as an operand or guard subject. The native side used to leave the
// local slot unwritten (the descriptor length was never materialized), so a later
// `n == 5` guard read the zeroed slot and took the false arm -- a silent read-0
// miscompile the interpreter never had. `s` views a fixed `[i32; 5]` through
// `.as_slice()`, so the length folds to 5 and the guard matches -> exit 5.

#[test]
fn runtime_slice_length_local_binding_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_LENGTH_LOCAL_BINDING_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-slice-len-local-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("slice length local binding canary should compile");
    assert_native_exit_code(
        &compilation,
        5,
        "slice-length local-binding canary",
        "the as-slice local should materialize its length into the value binding",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A runtime slice PARAM `.len` read into a LOCAL `usize` binding (`let n = s.len`).
// The `.len` place resolver reports the descriptor's low 4-byte len word, so a
// wider 8-byte `usize` slot failed the exact-size copy and the write was dropped
// (the local read 0). The descriptor holds the full 8-byte len, now read at the
// target's width. `s` views a fixed `[i32; 6]`, so `n == 6` matches -> exit 6.

#[test]
fn runtime_slice_length_local_param_binding_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SLICE_LENGTH_LOCAL_PARAM_BINDING_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-slice-len-param-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("slice length local param binding canary should compile");
    assert_native_exit_code(
        &compilation,
        6,
        "slice-length parameter-binding canary",
        "the slice parameter should materialize its full-width length",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A literal-bounded SUBSLICE `.len` read into a LOCAL binding in a value position
// (`let sub = self.arr[1..4]; let n = sub.len`). The subslice binding folds to the
// inline `(self.arr[1..4]).len` (no runtime descriptor slot), which the value-write
// resolver used to drop -> the local read 0. The window length `4 - 1 = 3` is a
// compile-time constant, now folded in the value-write path. `n == 3` matches -> 3.

#[test]
fn runtime_subslice_length_local_binding_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SUBSLICE_LENGTH_LOCAL_BINDING_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-subslice-len-local-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("subslice length local binding canary should compile");
    assert_native_exit_code(
        &compilation,
        3,
        "subslice-length local-binding canary",
        "the literal-bounded subslice should materialize its window length",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// The INLINE subslice `.len` (no `let sub` binding): `let n = (self.arr[1..4]).len`.
// Native folds the window length 3; the interpreter used to reject it ("range
// expression outside index position") because a member on a non-place receiver
// resolved as a place and hit the raw range -- it now evaluates the receiver as a
// value and reads `.len` off it. Both engines agree -> `n == 3` matches -> exit 3.

#[test]
fn runtime_inline_subslice_length_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INLINE_SUBSLICE_LENGTH_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-inline-subslice-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("inline subslice length canary should compile");
    assert_native_exit_code(
        &compilation,
        3,
        "inline subslice-length canary",
        "the inline literal-bounded subslice should fold to its window length",
    );

    let _ = fs::remove_dir_all(&build_dir);
}

// A FIXED-ARRAY base subslice with a RUNTIME (machine-field) END bound to a
// LOCAL: `let sub = self.arr[1..self.hi]`. The emitter seeds the local's slot
// with the whole-array descriptor (ptr = &arr, len = declared length) and
// shrinks it IN PLACE with the runtime end. Was fenced before this lowering.
// hi=4 -> window [1,4) -> len 3 -> exit 3.

#[test]
fn runtime_end_fixed_array_subslice_local_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_END_FIXED_ARRAY_SUBSLICE_LOCAL_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-rt-end-subslice-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-end fixed-array subslice local canary should compile");
    assert_native_exit_code(
        &compilation,
        3,
        "runtime-end fixed-array subslice canary",
        "the runtime end should shrink the seeded fixed-array descriptor",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The ELEMENT-READ face of the fixed-array runtime-END subslice local: the
// seeded-then-shrunk descriptor's POINTER must be element 1's address, not just
// a correct length. `sub = self.arr[1..self.hi]` over 10,20,30,40,50 -> a
// guarded `sub[0]` read through the descriptor -> 20.

#[test]
fn runtime_end_fixed_array_subslice_element_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_END_FIXED_ARRAY_SUBSLICE_ELEMENT_EXIT);
    let scratch =
        std::env::temp_dir().join(format!("omega-rt-end-subslice-elem-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-end fixed-array subslice element canary should compile");
    assert_native_exit_code(
        &compilation,
        20,
        "runtime-end subslice element canary",
        "the seeded and shrunk descriptor should point at the first subslice element",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A machine-field FIXED array's `.len` as a dispatch-guard comparison OPERAND
// against a runtime value (`hi <= self.arr.len`): the guard-normalization fold
// rewrites the constant length to its literal -> CompareStaticValue. Previously
// it classified as a runtime compare with no right storage and died at emission.
// hi=4 vs len 5 -> true arm -> exit 7.

#[test]
fn guard_fixed_array_len_operand_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::GUARD_FIXED_ARRAY_LEN_OPERAND_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-guard-arr-len-{}", std::process::id()));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("guard fixed-array len operand canary should compile");
    assert_native_exit_code(
        &compilation,
        7,
        "fixed-array length-guard canary",
        "the fixed-array length operand should fold before dispatch",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// The FULL runtime-bounded fixed-array subslice arc: both bounds runtime params,
// the dominating guard `lo <= hi && hi <= self.arr.len` lowers (fixed-array
// `.len` operand fold) AND discharges the prover's subslice obligations, and the
// true arm passes `self.arr[lo..hi]` as a slice argument through the
// seeded-then-shrunk descriptor. len 3 -> exit 3.

#[test]
fn runtime_bounded_fixed_array_subslice_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_FIXED_ARRAY_SUBSLICE_ARG_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-rt-bounded-subslice-arg-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("runtime-bounded fixed-array subslice arg canary should compile");
    assert_native_exit_code(
        &compilation,
        3,
        "runtime-bounded subslice-argument canary",
        "the guarded runtime bounds should produce the exact slice argument window",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier builder/concat, native: `self.text =
// "Room " + self.label` materializes into the target carrier's inline storage --
// the first literal initializes it, then the source carrier's content is appended
// (running offset + running len). `self.text == "Room A1"` matches -> exit 70.

#[test]
fn runtime_bounded_carrier_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_CONCAT_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-concat-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier concat canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bounded carrier-concat canary",
        "the owned bounded carrier should materialize the concatenation inline",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned `[u8; N] in Utf8` carrier MULTI-SEGMENT concat into a `&mut` OUT-PARAM
// (the dungeon render-line shape): `out_line = "== " + self.label + " =="` writes
// across a machine boundary into a borrowed carrier -- init literal, append the
// source carrier, append the trailing literal at the running length. Exits 70.

#[test]
fn runtime_bounded_carrier_alias_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_ALIAS_CONCAT_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-alias-concat-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier alias concat canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bounded carrier alias-concat canary",
        "the multi-segment concatenation should materialize through the borrowed carrier",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// #66 owned carrier concat with a FRAME-LOCAL source: `out_line = "== " + src +
// " =="` where `src` is a `let`-local carrier read from the runtime frame base.

#[test]
fn runtime_bounded_carrier_local_source_concat_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_CARRIER_LOCAL_SOURCE_CONCAT_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-bounded-carrier-local-source-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("bounded carrier local source concat canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "bounded carrier local-source concat canary",
        "the frame-local carrier source should materialize into the borrowed output",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// Carrier sibling of the slice-view-element test: a value-call guards on the
// element's CARRIER field (`room.label == "Gate"`, room = r[0], r =
// self.rooms.as_slice()). Carrier RECOGNITION now traces the elided local and
// sees through the as_slice view to resolve the field descriptor against the
// underlying array; before, the carrier `==` failed to lower (the arm was
// poisoned). Exits 70.

#[test]
fn runtime_value_call_slice_view_carrier_guard_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SLICE_VIEW_CARRIER_GUARD_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-slice-view-carrier-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-call slice-view carrier guard canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "slice-view carrier-guard canary",
        "the value call should resolve the carrier field through the slice view",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A value-call forwarded a SLICE-VIEW element by value (`read(r[0])`, r a local
// `self.rooms.as_slice()`): the body reads `room.id` through the BranchParameter
// alias `room = r[0]` -> `(self.rooms.as_slice())[0].id`. The place resolver now
// sees through the as_slice view AND traces the elided local to its initializer
// so the element resolves against the underlying array; before, it read a zero
// slot and the call returned 0. Exits 70.

#[test]
fn runtime_value_call_slice_view_element_arg_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_VALUE_CALL_SLICE_VIEW_ELEMENT_ARG_EXIT);
    let scratch = std::env::temp_dir().join(format!(
        "omega-value-call-slice-view-elem-{}",
        std::process::id()
    ));

    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("value-call slice-view element canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "slice-view element-argument canary",
        "the value call should resolve the forwarded element against the underlying array",
    );

    let _ = fs::remove_dir_all(&scratch);
}

// A `let`-local capturing `self.vm.sp + 1`, where `self.vm.sp` is reassigned
// before the local is forwarded through a nested dispatch (try_push1 -> push1).
// Argument materialization used to inline-fold the local back into its
// initializer and re-evaluate it AFTER the field was overwritten, so a deeper
// substate's guard saw the wrong value and branched into the wrong arm. The fix
// keeps the captured slot. Exits 70 (both pushes land: stack[0]=3, stack[1]=4).

#[test]
fn runtime_linear_search_early_exit_canary_runs() {
    // Linear search with EARLY loop exit: scan for `target`, leave the loop the instant it's found
    // (each element read into a field first, then compared). arr=[3,7,12,18,5], target=12 -> index 2.
    let canary = pass_canary(fixture_roster::RUNTIME_LINEAR_SEARCH_EARLY_EXIT);
    let scratch = std::env::temp_dir().join(format!("omega-linear-search-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let compilation = compile_rooted_canary_for_native_host(&canary, scratch.clone())
        .expect("linear search early exit canary should compile");
    assert_native_exit_code(
        &compilation,
        70,
        "linear-search early-exit canary",
        "the search should stop when it finds the target at index two",
    );
    let _ = fs::remove_dir_all(&scratch);
}

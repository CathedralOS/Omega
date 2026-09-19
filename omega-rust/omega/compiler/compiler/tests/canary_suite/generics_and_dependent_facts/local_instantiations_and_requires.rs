use super::fixture_roster;
use crate::{
    Command, check_canary, compile_reviewed_repository_fixture,
    compile_rooted_canary_for_native_host, fail_canary, fs, interpret, pass_canary,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_generic_let_local_instantiations_exit_canary_runs() {
    // Phase-1 type-position polish: two distinct instantiations of Box<T> as
    // LET-LOCALS (Box<i32> + Box<bool>), not fields. The desugar now scans
    // machine-body type positions (let-locals, params, returns), not just data
    // fields, so the 2nd instantiation no longer poisons the layout. exit 30.
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_LET_LOCAL_INSTANTIATIONS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-genlet-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic let-local instantiations canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic let-local instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic let-local instantiations canary should run");
    assert_eq!(
        output.status.code(),
        Some(30),
        "expected two coexisting generic let-local instances (exit 30), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_domain_instantiations_exit_canary_runs() {
    // Phase 1 domain-arg extension: two DOMAIN-CARRYING instantiations of
    // `Box<T>` (`Box<i32 in Wrapping>` + `Box<u8 in Wrapping>`) coexist. Each
    // argument carries an arithmetic domain, so before the slug extension both
    // were skipped (non-plain-Named) and fell to the one-slot poison path. Now
    // each slugs distinctly into its own synthetic record with the domain riding
    // the substituted field. exit 42.
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_DOMAIN_INSTANTIATIONS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gen-domain-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("domain-arg generic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("domain-arg generic instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("domain-arg generic canary should run");
    assert_eq!(
        output.status.code(),
        Some(42),
        "expected two coexisting domain-carrying generic instances (exit 42), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_array_max_and_sum_exit_canary_runs() {
    // Find the max and the sum of an array in one pass: an indexed read bound to a local, a
    // reduction (`total += v`), and an element comparison via the sound local-bind pattern
    // (`transition v > self.mx`). arr = [30,50,70,20,60,10] -> max 70, sum 240, both checked -> 70.
    let canary = pass_canary(fixture_roster::RUNTIME_ARRAY_MAX_AND_SUM_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-max-sum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("array max-and-sum canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("array max-and-sum canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("array max-and-sum canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a one-pass max+sum reduction to compute correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_reduction_loop_exit_canary_runs() {
    // Array reduction with a runtime index (`self.sum = self.sum + self.arr[self.i]`) in a loop --
    // an indexed read as an accumulation operand, the sum/reduce primitive. Sums [5,10,15,20,8,12]
    // = 70. The index bound is proven by the loop guard.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_REDUCTION_LOOP_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-indexed-reduce-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed reduction loop canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed reduction canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed reduction loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an array reduction over a runtime index to sum correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_rmw_loop_exit_canary_runs() {
    // Read-modify-write at a runtime index (`self.arr[self.i] = self.arr[self.i] + 10`) in a loop
    // -- the count/accumulate primitive, enabled by the machine-indexed binary write accepting an
    // indexed read as its value operand. Fills [0..4], increments each by 10 -> sum 60; a non-zero
    // `marker` after the index field guards the 32-bit index load and must survive -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_RMW_LOOP_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-indexed-rmw-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed RMW loop canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed RMW canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed RMW loop canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected read-modify-write at a runtime index to increment each element correctly (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_computed_indexed_write_exit_canary_runs() {
    // A computed value written straight into a runtime-indexed machine array element
    // (`self.arr[self.j] = self.j * 10`, no field temp). A non-zero `marker` field sits right
    // after the index field, guarding the 32-bit zero-extending index load (a 64-bit load would
    // pull `marker` into the index's high dword and store out of bounds). Fills [0..40] -> sum
    // 100 and marker survives -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_COMPUTED_INDEXED_WRITE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-computed-indexed-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("computed indexed-write canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("computed indexed-write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("computed indexed-write canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a computed value stored straight into a runtime-indexed element to fill correctly and not corrupt the neighbouring field (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_const_product_index_exit_canary_runs() {
    // R0 of the dependent-types ladder: the direct row-major spelling
    // `pixels[y * 4 + x]` (two-level computed index, const multiplier) in
    // read + write + ranged-param positions -- interval product discharges
    // the bound; the depth-2 hoist lowers it by slot. Pinned the former
    // silent-ZII miscompile.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_CONST_PRODUCT_INDEX_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-nested-product-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested const-product index canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested const-product index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested const-product index canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the row-major `pixels[y * 4 + x]` spelling to read/write the right element in every leg (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_hoisted_index_write_exit_canary_runs() {
    // A runtime value written through a hoisted computed index rides the
    // value local's frame slot + the storage-to-indexed copy (the
    // value-side slot carve-out in the former source-shaped backend).
    let canary = pass_canary(fixture_roster::RUNTIME_HOISTED_INDEX_WRITE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-hoisted-write-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("hoisted-index write canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("hoisted-index write canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("hoisted-index write canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the runtime value to land through the hoisted index (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_let_mut_reassign_exit_canary_runs() {
    // `let mut` reassignment reads the NEW value (slot-backed, never folded).
    let canary = pass_canary(fixture_roster::RUNTIME_LET_MUT_REASSIGN_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-let-mut-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("let-mut canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("let-mut canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("let-mut canary should run");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected the reassigned mut local to read 2 (exit 2), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_tuple_matrix_exhaustive_exit_canary_runs() {
    // ch4 tuple-subject transitions: covering bool matrices dispatch with
    // no `_ ->` arm.
    let canary = pass_canary(fixture_roster::RUNTIME_TUPLE_MATRIX_EXHAUSTIVE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-tuple-matrix-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("tuple-transition canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("tuple-transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("tuple-transition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the (true, _) arm to dispatch (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sum_tuple_matrix_exhaustive_exit_canary_runs() {
    // Multi-subject case patterns are proved over the Cartesian product; a
    // pure case-union domain contributes its finite subset to one axis.
    let canary = pass_canary(fixture_roster::RUNTIME_SUM_TUPLE_MATRIX_EXHAUSTIVE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-sum-tuple-matrix-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "sum-tuple transition canary should compile from its authored root without a `_` arm",
    );
    let executable = compilation
        .checked_native_executable_path()
        .expect("sum-tuple transition canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum-tuple transition canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the (Horizontal, _) arm for its second member (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_tuple_case_destructure_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TUPLE_CASE_DESTRUCTURE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-tuple-destructure-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("tuple case-destructure canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("tuple case-destructure canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("tuple case-destructure canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected both bound payloads to reach sum (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_param_range_exit_canary_runs() {
    // R1a: a state parameter ranged by a self FIELD (`i: u32
    // [0..=self.count]`) -- caller-proved at every transition, callee index
    // proofs through the substituted store-enforced high; the exclusive
    // sugar leg rides a strict `<` guard.
    let canary = pass_canary(fixture_roster::RUNTIME_DEPENDENT_PARAM_RANGE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-param-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent param-range canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dependent param-range canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dependent param-range canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dependent-ranged parameter to prove at the caller and index at the callee (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_product_index_exit_canary_runs() {
    // R0 x R1a composition: dependent-ranged params feed the row-major
    // product index with runtime arguments; both the overflow proof and the
    // hoist's temp range read substituted intervals.
    let canary = pass_canary(fixture_roster::RUNTIME_DEPENDENT_PRODUCT_INDEX_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-product-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent product-index canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dependent product-index canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dependent product-index canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the dependent product index to read the right element (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_subtract_exit_canary_runs() {
    // The relational subtraction rule: `self.count - i` proves non-negative
    // in exact u32 from i's dependent atom (capacity-minus-used).
    let canary = pass_canary(fixture_roster::RUNTIME_DEPENDENT_SUBTRACT_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-subtract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent subtract canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dependent subtract canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dependent subtract canary should run");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected `self.count - i` to prove and compute 8 - 6 (exit 2), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dependent_ordering_chain_exit_canary_runs() {
    // The minted in-callee ordering (`k <= self.count`) chains with a
    // dominating `count < 5` guard to discharge an index the substituted
    // range alone cannot.
    let canary = pass_canary(fixture_roster::RUNTIME_DEPENDENT_ORDERING_CHAIN_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-dependent-ordering-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dependent ordering-chain canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("dependent ordering-chain canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dependent ordering-chain canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the ordering chain to discharge the index (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_requires_subtract_exit_canary_runs() {
    // Channel (b): a machine-level `requires self.a <= self.b` proves
    // `self.b - self.a` in exact u32 (machine-wide field preservation).
    let canary = pass_canary(fixture_roster::RUNTIME_REQUIRES_SUBTRACT_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-requires-subtract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("requires-subtract canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("requires-subtract canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("requires-subtract canary should run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected the requires-ordered subtraction to prove and compute 0 (exit 0), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_requires_guarded_call_exit_canary_runs() {
    // The requires loop closed: a dominating arm guard proves the callee's
    // requires at the call site; the requires proves the subtraction inside.
    let canary = pass_canary(fixture_roster::RUNTIME_REQUIRES_GUARDED_CALL_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-requires-guarded-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("requires guarded-call canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("requires guarded-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("requires guarded-call canary should run");
    assert_eq!(
        output.status.code(),
        Some(6),
        "expected the guarded requires call to prove and compute 9 - 3 (exit 6), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sibling_len_index_exit_canary_runs() {
    // Buffer::get: `index: u64 [0..items.len]` -- caller-guarded, callee
    // indexes guard-free through the minted prove_index fact.
    let canary = pass_canary(fixture_roster::RUNTIME_SIBLING_LEN_INDEX_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-sibling-len-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sibling-length canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("sibling-length canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sibling-length canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the sibling-length index to read the seeded element (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bounded_product_index_exit_canary_runs() {
    // R3: runtime dims coupled only by `requires rows * cols <= 12`; the
    // product rule store-proves the ranged temp and the index rides it.
    let canary = pass_canary(fixture_roster::RUNTIME_BOUNDED_PRODUCT_INDEX_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("bounded-product canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None, "should interpret cleanly");
    assert_eq!(
        interpreted.exit_code, 7,
        "interpreter must preserve the bounded-product index result"
    );
    let build_dir =
        std::env::temp_dir().join(format!("omega-bounded-product-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bounded-product canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("bounded-product canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bounded-product canary should run");
    assert_eq!(
        output.status.code(),
        Some(7),
        "expected the coupled product walk to read the seeded element (exit 7), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_depend_mapping_exit_canary_runs() {
    // M2 blocker 3: build.omg depend rows map aliases for use-resolution.
    let canary = pass_canary(fixture_roster::RUNTIME_DEPEND_MAPPING_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-depend-map-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("depend-mapping canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("depend-mapping canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("depend-mapping canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the aliased use to reach the depended const (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_core_roster_ops_exit_canary_runs() {
    // N4 roster slice: core add/mul (cross-machine composition) + generic
    // recursive Seq<T> + a program-side structural length lemma.
    let canary = pass_canary(fixture_roster::RUNTIME_CORE_ROSTER_OPS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-core-roster-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("core roster ops canary should compile from its authored root");
    let executable = compilation
        .checked_native_executable_path()
        .expect("core roster ops canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("core roster ops canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the roster machines to validate and the program to run (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn saved_local_slice_length_requires_canaries() {
    // A state-local `let` slice binding is not a formal, but its produced
    // length is exact caller evidence at the call boundary: the callee's
    // `requires items.len <= k` discharges from the saved observation (3)
    // rather than from literal actuals alone.
    let canary = pass_canary(fixture_roster::SAVED_LOCAL_SLICE_LENGTH_COMPILE);
    check_canary(&canary).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    // The saved observation is exact, not permissive: the same produced length
    // must still reject when the bound argument is below it.
    let rejected = fail_canary(fixture_roster::SAVED_LOCAL_SLICE_LENGTH_UNMET_REJECTED);
    let expected = fs::read_to_string(rejected.join("expected.txt"))
        .expect("saved-local-length fail canary should carry expected.txt");
    let diagnostics =
        check_canary(&rejected).expect_err("an unmet saved local slice length should be rejected");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim()),
        "saved-local-length fail canary should contain {:?}:\n{combined}",
        expected.trim()
    );
}

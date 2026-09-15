use super::fixture_roster;
use crate::{Command, compile_rooted_canary_for_native_host, executable_name, fs, pass_canary};

#[test]
fn runtime_indexed_through_guard_chain_exit_canary_runs() {
    // An index bound carried across a CHAIN of convergent-arm guards (`d<0 {true->t
    // _->t}`) that neither name nor rewrite x. Before convergent arms were treated as
    // a single unconditional predecessor, each guard split dropped the bound. Compiling
    // + reading arr[3]=70 -> exit 70 confirms the bound survives the chain.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_THROUGH_GUARD_CHAIN_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_BINARY_SEARCH_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_TWO_POINTER_PALINDROME_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_STRUCT_ARRAY_FIELD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_ENUM_GRID_SCAN_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_TWO_INDEXED_READS_BINARY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_STRUCT_FIELD_TEMP_ARITH_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_STRUCT_WRITE_LOOP_EXIT);
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
    let canary = pass_canary(fixture_roster::STD_OPTION_RUNTIME_MATCH_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_READ_THEN_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_ROW_CONST_COLUMN_WRITE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_ARRAY_CONST_INDEX_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_WHOLE_ARRAY_VALUE_COPY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_WHOLE_STRUCT_VALUE_COPY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_RULE90_AUTOMATON_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FIXED_ARRAY_FIELD_GUARD_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_FIXED_ARRAY_FIELD_VALUE_EXIT);
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
    let canary = pass_canary(fixture_roster::FIXED_ARRAY_ELEMENT_GUARD);
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

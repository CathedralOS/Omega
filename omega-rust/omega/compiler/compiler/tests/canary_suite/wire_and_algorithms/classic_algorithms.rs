use super::fixture_roster;
use crate::{Command, compile_rooted_canary_for_native_host, executable_name, fs, pass_canary};

#[test]
fn runtime_newton_sqrt_exit_canary_runs() {
    // Newton's method for a square root (an iterative numerical algorithm): x <- (x + S/x)/2
    // over f64, six iterations from 1.0 on S=2.0 -> sqrt(2) ~= 1.41421; checks
    // 1.414 < x < 1.415 -> exit 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NEWTON_SQRT_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_MONTE_CARLO_PI_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_GCD_EUCLID_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_RPN_EVALUATOR_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_ACTIVITY_SELECTION_GREEDY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_MAZE_PATHFIND_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_NQUEENS_BACKTRACKING_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_COIN_CHANGE_DP_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_BFS_TRAVERSAL_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_HASH_TABLE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_MATRIX_MULTIPLY_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_RING_BUFFER_QUEUE_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_BUBBLE_SORT_EXIT);
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
    let canary = pass_canary(fixture_roster::RUNTIME_2D_TRANSPOSE_EXIT);
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

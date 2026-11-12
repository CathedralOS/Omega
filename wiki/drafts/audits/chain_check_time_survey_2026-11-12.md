# Expression depth survey (handed off mid-slice, 2026-11-12)

The survey stopped partway. Part (a), the time scaling of operator chains, is
measured and attributed. Part (b), overflow at 8 MiB stacks, was NOT measured:
no 8 MiB probe ran. One uncommitted edit exists. It was built and probed for
timing only. No tests, clippy, fmt or corpus were run on it.

## State of the worktree (left as is, on the coordinator's instruction)

- Worktree: `/home/user/Omega/.agents/worktree/worker-1`
- HEAD is `07d0248e6`, a cherry-pick of the probe patch `5b369baf3` from
  `origin/work/probe-stack-sizes`. It sits on `origin/main` `0e382613f`.
  **That commit must be dropped. It must never ship.** It is not the wip
  commit `a5dd1ad26` that also sits on that branch.
- Uncommitted: `omega-rust/psi/compilation/validation/src/expression_types_and_fields.rs`
  (+84/-28).
- Binaries in `scratchpad/bin/`. Both are dev profile, opt-level 1, with the
  probe patch applied:
  - `omega-probe-base`: origin/main plus the probe patch.
  - `omega-probe-fix`: the same, plus the uncommitted edit.
- New probe programs in `scratchpad/deep/`: chain500, chain1000, chain5000,
  paren5000, paren20000, prefix20000 (`~`×20000), neg20000 (`-`×20000).
  The earlier ones are chain300, chain2000, chain20000, paren300 and paren1000.
- Build time on this machine is 19 min cold for the probe build and 10.5 min
  for the incremental build after the validation edit.

## (a) Time scaling on `let x: i32 = 1+1+...+1;` at default 256 MiB stacks

These are `omega --check --timings` runs. Psi 05 includes Psi 04 preparation.

| chain | base total | base Psi 05 | fix total | fix Psi 05 |
| ---: | ---: | ---: | ---: | ---: |
| 300  | 33.8 s  | 31.9 s | 18.3 s | 16.3 s |
| 500  | 143 s   | n/a    | 62.2 s | 60.5 s |
| 1000 | >~10 min (killed) | | killed when the handoff came, not measured | |
| 2000 | >10 min (earlier session) | | not run | |

Psi 01–03 stay well under 200 ms at every size. All of the super-linear cost
is in Psi 04 preparation and Psi 05 validation. On the base build, going from
300 to 500 terms (1.67× longer) takes about 4.3× as long, which is roughly
cubic.

### Where the time goes

This comes from gdb sampling (`gdb -batch -p PID -ex "thread apply all bt"`),
using 4 samples on chain1000 and 12 on chain500. **100% of samples** were
inside
`typed_trees::type_system::closed_numeric::evaluate_value`, reached through
`has_anonymous_numeric_results`, at
`omega-rust/psi/compilation/representations/typed-trees/src/type_system.rs:69,180`.

Its callers in the samples were:
- `validation::...::result_type::schematic_type_parameter_operand`
  (`validation/src/expression_types_and_fields.rs`, around line 6050 after
  the edit)
- `typed_trees::operator::has_builtin_spelled_expression_meaning`, reached
  from `result_type::binary_result`

Each of these sat under a recursive
`validation::expression_types_and_fields::expression_types::result_type::result_type`,
up to 457 frames deep. That function is at line ~4840 before the edit and
~4895 after it.

The outer drivers were:
1. Psi 04, about 3 of the 12 samples:
   `typed_trees_to_typed_trees::preparation::selections::bind_pre_specialization_authored_selections`
   → `validation::operators::uses::collect_pre_specialization_operator_selections`
   (`validation/src/operators.rs:1887`). It calls
   `expression_result_type_reference` for both operands of **every** binary
   node in a state. It runs on a raw `TypedTrees`, whose
   `checking_memos()` is `None`, so nothing is memoized here.
2. Psi 05, about 9 of the 12 samples:
   `validation::recasts::validate_recasts` → `ValueOccurrenceVisitor::collect_expression`,
   which is itself recursive and appeared ~400 frames deep →
   `recasts::pending_views::validate_operand_consumptions::{closure}` →
   `expression_result_type_reference`, called per node.

### The three multiplied factors behind the cost

1. **The outer walkers type every node.** Both drivers above call
   `result_type` once per expression, so there are n calls.
2. **`result_type` re-walks the operand subtree on each call.** Its memo
   (`CheckingMemos::expression_result_types` / `parameter_result_types`)
   stored a `None` only when `active.is_empty()`. An anonymous literal
   subchain types as `None`, so inner results were never kept. Each call
   therefore costs O(k). There is no memo at all in Psi 04, which uses raw
   `TypedTrees`.
3. **At each binary node, `has_anonymous_numeric_results` re-evaluates the
   whole operand subtree.** It runs `evaluate_value` with BigRational
   arithmetic, which allocates on every operation. `evaluate_value` also
   guards against cycles with `active.contains(&expression)` on a `Vec` of
   enclosing nodes. That check is O(depth) per step, so one evaluation costs
   O(k²) comparisons. `result_type`'s own `active.contains` has the same
   O(depth) cost.

The combined cost is about n·Σk ≈ n³ bignum operations plus about n⁴
comparisons. Fixing any single factor leaves the chain super-linear.

## The uncommitted edit (built, timed only, otherwise untested)

The edit is in `validation/src/expression_types_and_fields.rs`, in
`mod result_type`. It replaces `active: &mut Vec<ExpressionHandle>` with
`ResultTypeWalk { enclosing, outermost_cut }` and `ResultTypeFrame`.

- **Cycle visits.** When the walk meets an expression that encloses it
  (a cycle), it records the outermost frame the cycle reached and answers
  `None`. Before, it answered `None` and recorded nothing.
- **What gets memoized.** On leave, a result is memoized when
  `result.is_some() || cut.is_none_or(|cut| cut >= frame.depth)`, meaning no
  cycle reached a frame that encloses it. The old rule was
  `is_some() || active.is_empty()`. The new rule stores everything the old
  rule stored, plus every `None` that does not depend on a cycle. Such a
  `None` is what a fresh walk from that expression would compute.
- **Entry points.** They pass `&mut ResultTypeWalk::default()` where they
  used to pass `&mut Vec::new()`.

Measured effect: chain300 went from 34 s to 19 s and chain500 from 143 s to
62 s, a 2.3× gain that grows with n. It is still super-linear, because
factor 3 remains and Psi 04 has no memo.

Not done on the edit: `mbx test -p validation`, clippy, fmt, corpus, and the
regression check. It has not been reviewed for whether a stored `None` can
interact with `CheckingQuery` cut scopes. `frozen_program_memos` returns the
query-scoped scratch memo there, and the old code already stored `Some` at
any depth.

## What remains for time, in pipeline order (proposals, not done)

1. **Psi 04 memo.** Run `collect_pre_specialization_operator_selections` over
   `validation::CheckingContext::new(program)` so it gets memos. Preparation
   already does this at `preparation.rs:2099` and `:2168`. Unverified risk:
   `CheckingContext` builds `call_frame_tables`, and raw `TypedTrees` returns
   `None` there, so results could differ.
2. **Factor 3, the general fix.** Compute closed anonymous numeric values
   once per expression rather than once per enclosing node. That means a
   bottom-up derived table of anonymous values, or threading the operands'
   anonymous values up through `result_type`. Complication: the result
   depends on `authored_declaration_selections`, which Psi 04 rebinds.
3. **The O(depth) cycle guards.** `evaluate_value` (`type_system.rs`, the
   `active.contains`) and `result_type` should check cycles only at the
   edges that can actually cycle: a Name to a const initializer, or a
   SavedSubject. Tree edges cannot cycle. There are about 149
   `active.contains(` sites in psi, so this is not a one-PR sweep.

## (b) and (c) Stack overflow at 8 MiB: NOT measured

No 8 MiB probe was run (`OMEGA_PROBE_STACK_MB_*=8`). What follows comes from
reading code and from deep frames seen while profiling at the default stack.
None of it is an observed overflow.

- **Parser.** Chains and prefix runs are loops: `parse_binary_chain_handle`
  at `01_tokens-to-syntax-trees/src/expressions.rs:584`, and
  `parse_unary_expression_handle` at `:690`, which collects prefixes in a
  `Vec`. Parentheses recurse through `parse_expression_handle_in` (`:407`),
  and `Input::deepen` (`tokens_and_storage.rs:415,493`) rejects them past
  `MAX_NESTING_DEPTH = 1024`. paren5000 and paren20000 are therefore expected
  to fail with that diagnostic, not overflow. Probing parentheses past 1024
  downstream needs the limit raised in a probe build.
- **Validation recursion seen on chain500.** Two recursive walkers appeared,
  both linear in chain length:
  - `result_type::result_type`, with a `try_map` frame between each level,
    457 levels at n=500;
  - `ValueOccurrenceVisitor::collect_expression`
    (`validation/src/value_custody_core.rs`, `executable_value_occurrences`),
    about 400 levels.

  Both are likely to overflow 8 MiB at chain sizes in the tens of thousands.
  This is unmeasured.

### Comparison with the walkers inventory (`scratchpad/walkers_map.md`, PR #2640)

- **Agrees.** It lists `parse_binary_chain_handle` and the unary prefix loop
  as iterative (D02, D03). It lists W01-1 as paren recursion that `deepen`
  bounds.
- **Disagrees.** It omits the two deepest walkers I observed:
  `validation::expression_types_and_fields::result_type::result_type`, a
  recursion over expression depth at 457 frames for n=500, and
  `ValueOccurrenceVisitor::collect_expression` in `value_custody_core.rs`.
  Neither appears under its W05 IDs, so its stage-05 list of 16 is incomplete.
- **Not verified.** Its W05-CORE claim that
  `queries::expression_traversal` is the shared child dispatcher.

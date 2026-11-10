# Does back-half constant folding refuse overflow and a zero divisor?

Yes, wherever the back half actually evaluates arithmetic on constants. Omega
01's sparse conditional constant propagation (SCCP) evaluates folds with the
same `IntegerType` methods the interpreter uses. The validator's replay does
too. Each case resolves as follows:

- **Exact overflow and any zero divisor.** The method returns `None`, so the
  operation is left unfolded.
- **Exact shift out of range.** Also `None`, so it is not folded.
- **Exact minimum ÷ −1.** Also `None`, so it is not folded.
- **Wrapping and saturating operations.** These fold to the value their
  policy defines.

There is a second kind of fold that evaluates nothing. Some Omega 01 identity
rules and some Omega 04 literal pair rules write a constant (`x/x → 1`,
`x%x → 0`, `0/x → 0`, `x % −1 → 0`) on the strength of an accepted obligation
fact alone. These remove a possible division by zero without looking at the
divisor. That is safe only if the checker's "yes" was right; per
`stage_trust.md`, nothing re-checks that fact.

Code: `<checkout>/` at
`80ce6c9c461`. Short names used below:

- `01/` = `omega-rust/omega/compilation/pipeline/01_abstract-operations-to-abstract-operations/src/`
- `sem/` = `.../representations/abstract-operations/optimization-unit/semantics/src/`
- `04/` = `.../pipeline/04_selected-instructions-to-selected-instructions/src/`
- `ao/` = `.../representations/abstract-operations/src/`
- `proposition.rs` = `omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs`

The integer semantics shared by every fold below are in `proposition.rs`:

- **Exact.** `checked_*` followed by `admits`, at `250-262`.
- **Exact remainder.** Minimum ÷ −1 gives None, at `673-690`.
- **Wrapping divide and remainder.** Minimum ÷ −1 gives MIN and 0
  respectively; a zero divisor gives None. See `692-724`.
- **Saturating divide and remainder.** Minimum ÷ −1 gives MAX and 0. See
  `726-753`.
- **Shift counts.** An exact count must be below the width (`806-818`). A
  wrapping count is taken modulo the width (`1200-1209`).

## Table

| Where a constant is computed | Operations folded | Overflow | Zero divisor | Shift out of range | Minimum ÷ −1 | file:line | Verification |
|---|---|---|---|---|---|---|---|
| Omega 01 SCCP binary rules | Exact, Wrapping or Saturating × add, subtract, multiply, divide, remainder; Exact or Wrapping shift left/right; bitwise and, or, xor | Exact: refused (`None`, so no candidate). Wrapping: folds the wrapped value. Saturating: folds the clamped value | refused for every policy | Exact: refused. Wrapping: the count is reduced modulo the width and the result folded | Exact divide and remainder: refused. Wrapping: MIN or 0. Saturating: MAX or 0 | rules `01/sccp.rs:205-228`; `None` skips the fold at `304-306`; `evaluate` `325-343` → `ao/integer_arithmetic.rs:182-207`, `231-253` | opened: corrected. b2_013 left every column UNDECIDED, and wrongly said the bitwise operations return a plain `IntegerValue`; they return `Option` (`proposition.rs:563-579`) |
| SCCP proof witness | (same folds) | the evaluation runs before the witness. A ProofCertified fold whose value is computable but whose obligation fact is missing returns an error through `?` | — | — | — | `01/sccp.rs:345-366` | opened. Not covered by any note. I did not trace what the pass manager does with that error |
| SCCP cast and unary rules | `IntegerExactCast`, `IntegerWiden`, `IntegerBitwiseNot` | refused when the cast or widen returns `None` | n/a | n/a | n/a | `01/sccp.rs:515-518`, `619-626` | opened: matches (b2_013) |
| SCCP comparison folds (constants, range against constant, range against range) | integer `==`, `<`, `<=`; boolean `not` and `==` | n/a: only comparisons. The ranges come from accepted proof facts, which are trusted | n/a | n/a | n/a | `01/sccp.rs:80-184`, `772-804`, `924-957` | opened `772-804`: matches. The other lines are not opened (from b2_013) |
| ScalarConstants analysis (the lattice) | none: it propagates literal facts and decides branches on constant booleans. It never evaluates an operation | n/a | n/a | n/a | n/a | `01/semantic_analyses.rs:1704-1870` | opened: matches (b2_014) |
| Validator replay of SCCP candidates | same set as SCCP, through the same `IntegerType` methods | a `None` makes the validator reject the candidate (`CandidateEvaluationMismatch`) | rejected | Exact: rejected. Wrapping: computed | as in SCCP | `sem/candidates.rs:8688-8785`, `9111-9140` | opened: corrected. b2_037 and b2_038 left minimum ÷ −1 UNDECIDED |
| Omega 01 proof-check elision: self and literal-divisor rules | `x/x → 1` (any policy; excludes signed 1-bit), `x%x → 0`, Exact `x−x → 0`, `x%1 → 0`, signed `x%−1 → 0` | n/a: nothing is evaluated | **not refused by the rule.** Each needs only an accepted obligation fact for the operation. `0/0` would become 1 | n/a | signed `MIN % −1 → 0` under every policy, including Exact, where the interpreter calls it malformed. This relies on the obligation | `01/proof_check_elision.rs:40-99` (needs a fact at `67-71`), `654-668`, `690-701`, `725-740`, `742-773`, `833-905` | opened: corrected. b2_008 said minimum ÷ −1 is "not mentioned"; the rule for signed `% −1` covers it |
| Omega 01 proof-check elision: identities | `x/1 → x`, `0/x → 0` and `0%x → 0` (all policies), Exact `x*0`, `x+0`, `x*1`, `x<<0`, and shifting the value 0 | n/a | `0/x` and `0%x` are folded without knowing x; they rely on the obligation fact | n/a | n/a | `01/proof_check_elision.rs:409-632` | opened: matches (b2_008) |
| Omega 01 GVN total identities | Wrapping and Saturating neutral or absorbing laws, multiply by zero, wrapping shift by zero, bitwise identities | n/a: compared against a known literal, never evaluated | divide and remainder excluded | n/a | n/a | `01/global_value_numbering.rs:1197-1206`, `1369` | opened: matches (b2_005) |
| Omega 01 `value_ranges.rs` | none: it turns constants and proof intervals into range facts | n/a | n/a | n/a | n/a | `01/value_ranges.rs` (a search for `evaluate`, `checked`, `exact_`, `wrapping`, `saturating` finds nothing) | searched by pattern, not read (b2_018) |
| Omega 04 `fold_immediate` | passes the literal through; for a constant result, the carrier maximum (saturating add) or 0; for copy and zero/sign extension, a mask | n/a: no arithmetic. The materialized value must pass `admits` | n/a | n/a | n/a | `04/literal_fold_rewrites.rs:3841-3867`, `3993-4003`, `3833-3838` | opened: matches (b2_054) |
| Omega 04 divide and remainder pair rules | `ExactDivideU64` by 1 → copy; `0 / x → 0`; `WrappingRemainderI64` by 1 → 0 and by −1 (`u64::MAX`) → 0; `0 % x → 0`; `SaturatingDivide` by 1 → copy and `0 / x → 0` | n/a | a zero divisor is refused: no rule matches divisor 0. A zero dividend is folded only if the consumer carries its own obligation in provenance | no shift rules | `x % −1 → 0` covers `MIN % −1` (the fault is discharged by the literal); this is correct for wrapping | `04/literal_fold_rewrites.rs:2830-2934`, `3245-3290`; obligation gate `3508-3524`, `1516-1521` | opened: matches (b2_054, b2_055) |
| Omega 04 saturating add and subtract rules | identity by 0; result forced to the maximum or 0 for an unsigned carrier | n/a: guarded by `assert!(!carrier.is_signed())` | n/a | n/a | n/a | `04/literal_fold_rewrites.rs:3101-3243` | opened: matches (b2_055) |

## What remains unknown

- What the pass manager does when the SCCP witness fails with a
  `RuleProposalError`: drop the candidate or fail the pass. See
  `01/sccp.rs:357`; not traced.
- When the validator replay in `sem/candidates.rs` runs: in every compile, or
  only in verification harnesses. Not traced.
- Whether `ExactRemainderU64` and the other Omega 04 rewrite files
  (`copy_and_extension_rewrites.rs`, `branch_and_compare_rewrites.rs`,
  `address_and_load_rewrites.rs`) hold literal folds. Those files were outside
  the brief and were not read.
- Whether an obligation fact for an Exact remainder always covers
  `MIN % −1`. If it does not, Omega 01 folds a case the interpreter rejects.
- `b2_031` (`optimization-unit/src/rewrite.rs`, lines 1-3000) is incomplete.
  `b2_032` and my reading of where SCCP candidates are built
  (`01/sccp.rs:1072-1118`) suggest `rewrite.rs` only builds candidates and
  computes nothing. I did not open `rewrite.rs` lines 1-3000.

## Notes review

There are 55 `b2` notes. Most correctly report "no folding in this file". I
checked the notes behind the rows: b2_005, 008, 013, 014, 018 (search only),
037, 038, 054 and 055.

- **Wrong on a detail (2 notes).**
  - b2_013 says the bitwise operations return a plain value; they return
    `Option`.
  - b2_008 says minimum ÷ −1 is not mentioned; the signed remainder by −1 rule
    covers it.
- **Correct but incomplete (3 notes).** b2_013, 037 and 038 left the central
  question (overflow, zero divisor, minimum ÷ −1) UNDECIDED because they did
  not open `ao/integer_arithmetic.rs` or `proposition.rs`. Those two files
  decide it.
- **Matched the code** in the rest of the checked set.

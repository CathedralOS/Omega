# Case-where equality

Case `Point(lo, hi) where lo == hi` makes `hi - lo` exact in `u64`, without
a guard or Wrapping. The fact-place requirement keeps this fact and operation
unchanged. **Acceptance is 8/9: the const-name place remains open.** Its current
closed-literal witness checks, but does not show that the case predicate is
essential. A green source gate does not close that missing acceptance.

This is source-check acceptance only. The original `main.omg` and its build
entry bindings are preserved. The nine place projects bind no `ProgramEntry`
and have no runtime oracle; no native execution is claimed.

```sh
omega --check samples/cli/facts/case_where_equality/main.omg
omega --check samples/cli/facts/case_where_equality/<place>/main.omg
omega --check samples/cli/facts/case_where_equality/<place>/invalid/main.omg
python3 tools/sample_gate.py --cli <omega> --filter cli/facts/case_where_equality --jobs 2
```

| Place | Equality-bearing subject | Positive | Reject twin |
| --- | --- | --- | --- |
| `local` | copied local interval | checks | missing equality: exact `u64` overflow |
| `field` | borrowed context field | checks | missing equality: exact `u64` overflow |
| `parameter` | explicit interval parameter | checks | missing equality: exact `u64` overflow |
| `call_result` | identity machine result | checks | missing equality: exact `u64` overflow |
| `match_join` | two distinct interval fields joined by a match | checks | missing equality: exact `u64` overflow |
| `const_name` | named `Point { lo: 4, hi: 4 }` constant | checks; not counted | constructor rejection only; no discriminating predicate-removal twin |
| `other_state` | interval forwarded to another state | checks | missing equality: exact `u64` overflow |
| `other_machine` | interval forwarded to another machine | checks | missing equality: exact `u64` overflow |
| `target_body` | four hosted target bodies | checks | four exact `u64` overflow diagnostics |

Eight twins remove only `where lo == hi`; the const twin changes only `hi: 4`
to `hi: 3` and pins construction enforcement. Its rejection is distinct from
subtraction rejection and does not qualify the const-name fact place. Expected diagnostic fragments are authored beside each
twin. Pass/fail mirrors live at
`tests/omega/{pass,fail}/facts/case_equality_through_<place>_compile`.

`Interval [copy]` explicitly permits the ordinary copies used by the place
programs. Unannotated owned data is affine even when all its fields are scalar;
the initial borrowed-storage transfer refusals were required ownership checks,
not arithmetic defects. The original root source remains unchanged.

On base `2ef2ab353498f0830f6023d8a023b63d21664cb9` (containing
`73ecbccba3`), eight place programs checked after that declaration. The const
place first rejected a complete `Point`/`Empty` dispatch as non-exhaustive, then
rejected exact subtraction after coverage was repaired. The compiler repair
compares complete aggregate dispatch subjects, binds substituted payload fields
to their exact case, and extends closed scalar projection through the complete
common/active constructor roster. It retains case predicates, ownership,
closed-sibling and Exact-policy obligations.

The focused corpus also checks constant-array dispatch, distinct aggregate axes,
common fields with same-named sibling payloads, and unsafe sibling arithmetic.
An additional maximum-`u64` construction probe still fails on the exact base
before this repair: its case predicate is not established. That existing wide
construction gap is outside this authored constant-4 witness and is not claimed
fixed by the source checks here.

## Const-name audit: acceptance remains open

The audit at `e0ddeec45d6b36ad90d1ace436b47d6b1741f67a` removes only the
case's `where lo == hi`, keeping the named constant, `4`/`4` payloads,
transition, state parameters, and exact `hi - lo` assignment. It still checks.
The existing const-name rejection therefore did not establish that the
transported predicate was needed. The earlier 9/9 completion claim was incorrect.

| Case predicate | Named constant payloads | Observed source result |
| --- | --- | --- |
| `lo == hi` | `lo: 4, hi: 4` | checks (retained positive) |
| absent | `lo: 4, hi: 4` | checks (`controls/literal_equal_without_predicate`) |
| absent | `lo: 4, hi: 3` | rejects `state take` exact `u64` subtraction (`controls/invalid_unequal_without_predicate`) |
| `lo == hi` | `lo: 4, hi: 3` | rejects constant case predicate (retained `invalid`) |

The two added controls differ only in `hi: 4` versus `hi: 3`. They establish
closed-scalar subtraction checking, not case-predicate transport. Corpus mirrors
are `pass/facts/case_const_equal_literals_without_predicate` and
`fail/facts/case_const_unequal_literals_subtraction_rejected`.

This limitation follows the current witness's representation. The
[constant contract](../../../../wiki/spec/language/constants.md) names an
immutable compile-time value and requests semantic evaluation; symbolic
mathematical definitions are a separate construct. The existing closed
constructor projection in
`omega-rust/psi/compilation/checking/src/value_custody/literals/record_projection.rs`
retains the landed payload scalars. Those known `4`/`4` values independently
prove exact subtraction. Under the
[case contract](../../../../wiki/spec/language/data_and_literals.md#case-constraints),
an omitted clause contributes true, and
[Exact arithmetic](../../../../wiki/spec/language/numeric_values.md#arithmetic-and-comparisons)
requires representability, not dependence on a particular proof premise.
Rejecting safe `4 - 4` merely because the optional clause is absent would change
those rules. Disabling scalar projection would hide valid evidence rather than
repair the compiler.

No new language restriction or impossibility theorem is asserted: this does not
rule out every spec-supported symbolic const-name witness. The remaining
obligation is to demonstrate the unchanged case-equality fact as essential to
`hi - lo` at const-name, with a control that removes only that fact and reaches
subtraction rejection. This materialized-literal route cannot distinguish it;
no qualifying replacement is established here. The eight other routes and the
landed compiler repairs are preserved. All evidence remains check-only.

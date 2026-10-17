# Case-where equality

Case `Point(lo, hi) where lo == hi` makes `hi - lo` exact in `u64`, without
a guard or Wrapping. The fact-place requirement keeps this fact and operation
unchanged. The nine source-check places include a named constant selected through
a static binder, whose generic body needs the case predicate. Each qualifying
twin removes only that predicate and reaches subtraction rejection.

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
| `const_name` | named `Point { lo: 4, hi: 4 }` constant through static `const INTERVAL: Interval` binder | checks | missing equality: exact `u64` overflow in the generic body |
| `other_state` | interval forwarded to another state | checks | missing equality: exact `u64` overflow |
| `other_machine` | interval forwarded to another machine | checks | missing equality: exact `u64` overflow |
| `target_body` | four hosted target bodies | checks | four exact `u64` overflow diagnostics |

All nine twins remove only `where lo == hi`, retaining the same `u64` carriers,
transition, state parameters, and exact `hi - lo` assignment. In the const twin,
the named `4`/`4` constructor remains valid, but it cannot strengthen the generic
body's declared contract. Expected diagnostic fragments are authored beside each
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

## Materialized-literal audit controls

The audit at `e0ddeec45d6b36ad90d1ace436b47d6b1741f67a` removes only the
case's `where lo == hi`, keeping the named constant, `4`/`4` payloads,
transition, state parameters, and exact `hi - lo` assignment. It still checks.
The existing const-name rejection therefore did not establish that the
transported predicate was needed. The earlier 9/9 completion claim for that witness was incorrect; the audit
correction correctly reopened acceptance before the static-binder repair below.

| Case predicate | Named constant payloads | Observed source result |
| --- | --- | --- |
| `lo == hi` | `lo: 4, hi: 4` | checks (`controls/literal_equal_with_predicate`) |
| absent | `lo: 4, hi: 4` | checks (`controls/literal_equal_without_predicate`) |
| absent | `lo: 4, hi: 3` | rejects `state take` exact `u64` subtraction (`controls/invalid_unequal_without_predicate`) |
| `lo == hi` | `lo: 4, hi: 3` | rejects constant case predicate (`controls/invalid_unequal_with_predicate`) |

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

No new language restriction or impossibility theorem follows from those
controls. The materialized-literal route cannot distinguish predicate necessity.

## Named constant through a static binder

`const_name/main.omg` retains the named `Point { lo: 4, hi: 4 }` value as
`NAMED_INTERVAL`. `Main::selected` explicitly selects it into
`Main::main<const INTERVAL: Interval>`. That body directly dispatches the const
name, forwards the same payload bindings into `take`, and assigns `hi - lo`.
It introduces no runtime parameter for the interval and adds no arithmetic guard.

The [generic contract](../../../../wiki/spec/language/generics.md#value-binders-and-const-requirements)
makes this binder statically supplied within its specialization. Generic bodies
check against their declared requirements: a concrete selection cannot silently
strengthen the API. Matching the binder's established `Point` case therefore
supplies the equality needed by the generic body. Removing only the predicate
rejects subtraction there even though the concrete `4`/`4` constructor is valid.
This demonstrates the fact through a const name without pretending literal
arithmetic depends on the predicate.

On base `866a955e52f8abcdce6d7b80e18fbbb772f51205`, the full named selection
rejected exact arithmetic in `Main::main$specialized$…` state `take`. The shared
declared-receiver lookup omitted static and runtime-capable value binders. Binding
their members through the authored carrier preserves exact payload identities
into specialization. Existing case/field owner checks and scalar projection stay
in place; no predicate or arithmetic obligation is relaxed. Forwarded const
selection and a runtime-capable value binder pin the same operation's composition.

An unused, uninstantiated binder-only scratch probe still reports its pre-existing
unresolved authored member selection. It does not supply the assigned named
constant/application journey and is not claimed repaired. The nine assigned
routes, all their predicate-erasure twins, and the four materialized controls
remain source checks only; no native execution is claimed.

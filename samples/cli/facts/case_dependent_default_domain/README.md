# Case-dependent default and named predicates

Case membership and the complete predicate recover the payload type and ordered bounds through the transported value.

Capability: **DOMAIN-DEFAULT-PREDICATE-UNIFICATION**. The row uses a generic
`pick<T>(Value<T>) -> T`, with fixed `i32` and `bool` case payloads. Matching
supplies the type equation; callers add no type test or equality guard.
`NamedValue in Typed<T>` has the same predicate over a carrier with no data
predicate, so its extractor depends on the named domain itself.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, carrying a closed generic
data application's complete payload predicate through construction, matching,
calls, stored values and materialized constants to its scalar consumers.

`Interval<32>` and `NamedInterval in Ordered<32>` express the same Empty-or-Range
predicate. The Range case carries `lo <= hi && hi <= 32` to `width_range`.
That consumer establishes `lo in AtMost<32>` and computes `hi - lo` directly.
Neither caller reconstructs the bound after transport.

Every place transports a Bundle containing both extractor applications and
both intervals. Default and named values have different payloads and widths:
42/true/4 versus -7/false/6, then -13/false/77 versus 23/true/77. Empty returns
77. Leading and trailing canaries detect damaged record or ABI transfers.
Erasing the named classifications to `NamedValue` must preserve Integer and
Boolean tags. A wrong branch, swapped payload, dropped field or stale return
home makes `main()` trap.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| selected implementation | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

The reject twins remove only `hi <= Bound` from `Interval<Bound>`'s Range
alternative. Construction still permits the authored values, but the public
extractor cannot promise the missing ceiling for every admitted input.
The [missing type equation](controls/missing_type_equation/main.omg) control
retains the earlier twins' obligation once: a Boolean payload cannot be
delivered as an arbitrary T without its type equation. Samples and their twins
are pinned under
`tests/omega/{pass,fail}/facts/case_dependent_default_domain_through_<place>_{exit,rejected}`.
Every positive binds all four hosted targets; the target-body place selects four
ordinary named implementations through its four entry roots.

The specification requirements exercised by the row are:

- **Establishment:** constructors and explicit qualifications prove the complete
  data or domain predicate. The logical subject has no runtime argument.
- **Identity and lifetime:** each application retains its exact T or Bound and
  its exact observed subject. A fact about another value or domain argument
  cannot justify this extraction.
- **Composition:** the nine transports preserve the same values, predicates,
  consuming operations and oracles; both join choices execute.
- **Soundness:** the missing ceiling rejects the interval consumer, and omitted
  type equality still rejects the universal extractor. Other
  controls cover invalid cases, inactive payloads, reversed bounds, mutation
  restoration, zero establishment and retired syntax.
- **Psi 05–07:** the complete source predicate supplies case-local proof
  consequences before proof erasure. Actual values still use ordinary tagged
  data, calls and subtraction; `--both` runs Terminal Psi independently.
- **Omega 00–02:** Build selects ProgramEntry normally. Bundle transfers and
  scalar returns use ordinary layouts and calling plans; domains add no data.
- **Omega 03–06:** signed and Boolean results, tag dispatch, canaries and
  caller-visible values expose instruction, register and frame mistakes.
- **Omega 07–09 and emission:** the native image must independently produce
  exit 0 and agree with Terminal Psi.

Acceptance command for each positive:

```sh
omega run --both samples/cli/facts/case_dependent_default_domain/<place>/main.omg
```

The baseline at `a91a8de86514f5b0ba71509b3af4d656916d6062` rejects both
reconstructed generic extractor probes with incompatible match arms and lost
return-type equality. The guarded named-domain probe additionally rejects its
case classification and exact subtraction. Raw diagnostics and subsequent
candidate observations are retained in `build/domain-default/` in this worktree.
This record does not claim native execution on hosts that were not observed.

The positive controls use the same predicates:

| Control | Runtime distinction |
| --- | --- |
| [guard_effects](controls/guard_effects/main.omg) | Four reached guards increment caller state, return false, then fall through to the matching payload consumer. |
| [restoration](controls/restoration/main.omg) | Invalid intermediate writes are repaired before default observation or named requalification; widths are 8 and 5. |
| [named_coverage](controls/named_coverage/main.omg) | Exact `Typed<bool>` and `Typed<i32>` applications each cover only their admitted case; numeric `Nonempty<32>` covers Range without a type equation. |
| [zero](controls/zero/main.omg) | Omitted Integer fields establish both default and named `i32` predicates and extract zero. |

The negative [controls](controls/) reject wrong default/named case-type pairs,
inactive payload reads, reversed bounds, default/named stale writes, stale type
facts after whole replacement, another subject's predicate, another generic
bound, default/named invalid zero, invalid constant qualification, and retired case-local `requires` syntax.
Their source copies and diagnostic fragments live in
`tests/omega/fail/facts/case_predicate_<control>_rejected`; the four positives
have native and interpreter exit oracles under the matching pass paths.

The interval's bound is an ordinary Const argument; its value remains 32, its
payloads and `hi - lo` operation are unchanged. This strengthens the existing
row for the generic-declaration transfer that wrongly rejects the migrated
`zero_case_predicate_entry_exit` customer's `amount * 10`. On optimized base
`0948d66c9cda2a14b141212ba98e6d126a08752d`, the otherwise unchanged local
program with `Interval<32>` rejects `width_range`'s requirement; the earlier
non-generic interval passed. The [other default argument](controls/other_default_argument/main.omg)
control admits `Interval<64>` with `hi = 40` and must still refuse a ceiling of
32. The hexadecimal canaries denote the earlier decimal values exactly.
Current run records retain their producer identities separately from the
historical qualifications below.

The generic restoration control writes an invalid intermediate upper endpoint
and repairs both endpoints before consumption. Its stale-write twin still
rejects the open invariant window. This carrier correspondence repair does not
close the parent's separate snapshot-based relational restoration, other
predicate productions, product/compiler migration or suffix retirement.

## Initial qualification

On Linux x86-64, the final optimized reference producer (`opt_level=1`, SHA-256
`138087bea8064f2d9a976ce88d128d9b80dd7a4e96d2a2d37330c257cdc2c903`)
passed all 39 direct observations: nine places, four positive controls, three
standalone extractor/bounds probes and the migrated borrowed payload sample
all exited 0 in both native execution and Terminal Psi; all nine reject twins
and thirteen negative controls rejected for their authored diagnostic fragments.
The const-name place uses actual constant Bundle literals. The guard control
observes all four reached guards' increments before fallback dispatch.

The worktree retains commands, exits and timings in
`build/domain-default/final-results.json`, the verified executable profile in
`final-producer.json`, and a raw `final-<case>.log` for each observation.
`canonical-context.json` retains the ratified specification passages and source
identities used for this implementation. The source-pair audit checks that each
twin removes only the Boolean type equation and that every corpus copy agrees.

The migrated borrowed-payload sample also has a separate negative observation:
its constructor rejects with `literal cannot prove the default domain`. Its
sample metadata and expected text use that complete-predicate diagnostic.

Recursive/indexed families retain their checked-encoding obligations. A separate
44-fixture comparison of intermediate candidate 14 against the frozen base found
no lost source acceptance or diagnostic fragment; all 44 already refused on that base. In particular,
`source_indexed_family_recursive_index_positive` still reports that nested
source-family recursion has no checked encoding. This row establishes the
ordinary predicate/type-equality path and does not assert that missing encoding
has been implemented.

The scoped checked corpus recorded 38 intended rows: 14 checked positives and
24 rejections with their expected fragments. The const-name, effectful-guard and
migrated borrowed-payload fixtures also recorded native/Terminal exit 0 and
agreement. `final-golden-audit.json` confirms that only those 38 checked rows and
three execution rows changed. The parser regression, scoped Clippy checks and
format check also succeeded.

The wider 92-fixture slice has no newly regressed rows or execution-oracle
failures, but its gate remains red for four existing nonsuccesses. The exact
six-fixture comparison against the frozen base reports zero losses and two
corrected rejection pins. Its base observations are retained separately from
the candidate observations; a baseline refusal does not prove the candidate's
different refusal reason.

| Wider fixture | Frozen base | Final candidate |
| --- | --- | --- |
| `case_where_payload_write_exit_open` | Rejects at the call; old expected text does not match. | Same refusal and old expected-text gap. |
| `source_indexed_family_wrong_payload_carrier` | Missing recursive encoding; old expected text does not match. | Same refusal and old expected-text gap. |
| `zero_case_predicate_entry_exit` | Incomplete structural field-store lowering. | Reaches an exact-arithmetic proof refusal. |
| `source_indexed_family_recursive_index_positive` | Missing recursive encoding. | Same missing encoding. |

Those four corpus rows were not repinned. The worktree retains
`candidate-21-corpus.json`, `final-recorded-corpus.json`, `final-both-corpus.json`,
`final-base-comparison.json` and the corresponding raw logs under
`build/domain-default/`. `final-assumption-review.json` records the challenged
identity, mutation, zero, coverage, guard-effect and constant-custody premises.


## Conflict integration

The resolved integration with main `52cce957d7d9a1b64517b160fb8ef7a3a7121009`
uses optimized producer SHA-256
`7b7ae7ff14df473a5d3c8986aec82513b2d48ff93441d3a2ca8f9fc51f23d4c7`.
All 64 focused observations pass: 29 programs exit 0 in both native execution
and Terminal Psi, and 35 negative controls reject with their authored fragments.
This reruns the initial 39 direct observations and migrated-payload negative,
then checks main's ten positive/ten negative generic-extraction fixtures and
four indexed-observation handback controls.

Main's generic transition rows now receive conditional type equations from the
same complete-predicate prefix judgment. An owned saved subject must retain the
exact independently admitted entry value and semantic application; a receiving
type supplies no premise. Closed-case impossibility remains canonical, and the
removed projected case clauses are not restored. The four reached guard effects
and restoration controls still execute on both engines. A separate unused
generic body returning the wrong terminal-call type also rejects.

The worktree retains `integration-4/results.json`, raw logs,
`integration-4-source-manifest.json`, `integration-4-source-digests.json`, and
`integration-4-review.json` under `build/domain-default/`. The 55 changed Rust
files pass formatting, and the owned diff passes whitespace checks. The broader
92- and 44-fixture comparisons above retain their original producers and were
not rerun after this integration.

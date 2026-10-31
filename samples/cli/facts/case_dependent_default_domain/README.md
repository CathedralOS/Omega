# Case-dependent default and named predicates

Case membership and the complete predicate recover the payload type and ordered bounds through the transported value.

Capability: **DOMAIN-DEFAULT-PREDICATE-UNIFICATION**. The row uses a generic
`pick<T>(Value<T>) -> T`, with fixed `i32` and `bool` case payloads. Matching
supplies the type equation; callers add no type test or equality guard.
`NamedValue in Typed<T>` has the same predicate over a carrier with no data
predicate, so its extractor depends on the named domain itself.

`Interval` and `NamedInterval in Ordered<32>` express the same Empty-or-Range
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

The reject twins remove only `T == bool` from `Value<T>`'s Boolean alternative.
Construction still permits the authored values, but the generic extractor
cannot deliver a Boolean payload as an arbitrary T. Samples and their twins
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
- **Soundness:** omitted type equality rejects the universal extractor. Other
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

## Observed qualification

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

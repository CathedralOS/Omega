# Transparent input predicates at selector calls

Inline exact IEEE value `0.25` guarantee refines public inclusive f64 interval
`[0, 0.5]`; the dyn call preserves the public interval for a precondition and observes `0.25`.

Owning task: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**, transparent
proposition aliases in requirement checking. This is another production of the
existing [dynamic IEEE fact](../README.md), following its
[public-result alias production](../alias_contract/README.md).

The input proposition `quarter(value)` is the checked mathematical definition
`value >= 0.25 && value <= 0.25`. `probe` receives that proposition on its exact
input and must prove `Fraction::get`'s public `public_interval(input, 0.0, 0.5)`
precondition before making the selector call. The implementation proves its own
quarter-valued result separately. `accept` consumes only the requirement's public
result interval. No caller guard re-establishes an input or result fact.

At fixed base `bfcffe6bcfcedb6fd60713e223408566c6c4e10d`, the optimized shared
producer refuses the frozen named-input program at both `quarter(0.25)` and the
public dynamic-call precondition. Expanding only `requires quarter(input)` into
its exact checked body runs with native exit 0 and Terminal exit 0. This proves
a transparent input-predicate transport gap, rather than a missing IEEE weakening
rule or a result-alias regression. Exact sources, producer digests, raw streams
and noninvalidated qualification receipts are retained in
`build/selection-dynamic-contracts/` and the source qualification record.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [positive](local/main.omg) | [weakened input](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [weakened input](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [weakened input](parameter/invalid/main.omg) |
| call_result | [positive](call_result/main.omg) | [weakened input](call_result/invalid/main.omg) |
| match_join | [positive](match_join/main.omg) | [weakened input](match_join/invalid/main.omg) |
| const_name | [positive](const_name/main.omg) | [weakened input](const_name/invalid/main.omg) |
| other_state | [positive](other_state/main.omg) | [weakened input](other_state/invalid/main.omg) |
| other_machine | [positive](other_machine/main.omg) | [weakened input](other_machine/invalid/main.omg) |
| target_body | [positive](target_body/main.omg) | [weakened input](target_body/invalid/main.omg) |

Every twin changes only `quarter`'s upper input bound from `0.25` to `0.75`.
The entry's input remains `0.25`, but `probe` must work for every admitted input.
It must reject the public precondition of `call dyn Fraction::get from probe`;
an entry-call or conformance refusal is not the intended control.

`Quarter` and `OtherQuarter` both return `0.25` but have distinct marker results
11 and 13. The match-join place invokes both selector-producing arms and checks
both markers, so choosing the fallback for both inputs traps. The field place
surrounds its selector with nonzero neighbors 17 and 29 and verifies them after
dispatch. The target place uses ordinary named implementations selected by four
explicit target-keyed Build rows; x86 entries expect marker 11, ARM entries 13.
The providers introduce no implicit receiver or data instance.

The [split-relation control](controls/single_relations/main.omg) proves the same
input through two separate one-relation aliases. The
[Boolean-disjunction control](controls/boolean_disjunction/main.omg) checks
existing checked Boolean equality and disjunction goals while retaining the
same dynamic input obligation; it does not claim general disjunctive-premise
entailment.

Pins are byte-identical sources and Build inputs under
`tests/omega/{pass,fail}/facts/dynamic_float_input_alias_through_<place>_*`.
Every positive declares Expected exit 0 and authors all four hosted ProgramEntry
bindings. Run each positive through `omega run --both <place>/main.omg`; check
the twin's authored diagnostic independently.

- **Establishment:** checked mathematical definitions retain selected IEEE
  relations and their exact literal format. The input alias establishes the
  public call precondition; implementation postconditions independently establish
  the public result interval. A wanted conclusion never supplies its premises.
- **Identity and lifetime:** alias captures substitute exact observed actuals.
  Selector and data lanes remain separate; no source spelling, stale storage
  read or implementation-private guarantee supplies an input fact.
- **Composition:** locals, fields, parameters, call results, both match arms,
  constants, nested machines, forwarding machines and Build-selected providers
  carry the same selector and predicate obligation. Both public result aliases
  and their inline production remain independent regression consumers.
- **Soundness:** input-bound twins reject for the selector requirement; IEEE
  NaN, signed zero, f32 rounding, selected operation meaning and the distinction
  between public and private guarantees retain their ordinary rules. A false
  ordered predicate grants no integer complement law.
- **Psi 06–08:** mathematical input/result propositions erase after checking;
  selected dispatch and ordinary f64 arguments/results remain in canonical
  Terminal Psi. Actual interpreter execution must agree with native exit 0.
- **Omega 00–02:** the existing selector word, record layouts, public slot call
  shapes and hosted entry rows remain unchanged. The target place checks ordinary
  Build-selected implementations without target-prefixed source families.
- **Omega 03–06:** indirect calls, f64 argument/result lanes and live selector
  preservation must retain the observed quarter and distinct runtime markers.
- **Omega 07–09 and emission:** native images must execute the selected callable
  and preserve selector neighbors; the authored crash oracles trap wrong values,
  branch choices and target selections. Bound target rows do not claim execution
  on hosts absent from the evidence below.

The parent remains open for its other contract mechanisms, operational-envelope
refinement, runtime finite-family inputs, product parity, table realization and
other-host native acceptance. This production closes only the demonstrated
transparent IEEE input-predicate transport requirement after its acceptance.

## Recorded acceptance

On Linux x86-64, the optimized producer built from
`d8105b3feb7899b2948107c1fbe5c6fe99555ef7` runs all nine positives with
**native exit 0 and Terminal Psi exit 0**. All nine twins reject at
`cannot prove requires contract for call dyn Fraction::get from probe`.
`build/advance-input-alias/sample-evidence.json` records the exact commands,
source hashes and both engine streams; `fixture-integrity.json` verifies the
one-bound twin change and byte-identical corpus pins.

The unchanged original frozen named-input and public-alias programs now run
with both exits 0, as does the inline control. A fresh authored match-join copy
changes only the second marker oracle from 13 to 12: native execution reports
signal 4 and Terminal execution reports `Trap`. This observes the runtime
falsifier independently of static rejection. Raw control streams are retained
in `build/advance-input-alias/`.

The immutable producer is `build/advance-input-alias/producers/r2/omega`,
SHA-256 `661ba42466b14f31b2af5e4546b1fae6de81e16b51751e7a7f4f484dbcc59df6`;
its corpus runner is SHA-256
`42d891a5c45561e26b37a176b7808e7ea978a50ddea0b4aba6c3da0f3b36cc5e`.
Both report optimization level 1. `mbx check -p validation` passes.
`mbx clippy -p validation --no-deps -- -D warnings` reports 202 existing crate
style errors; every located diagnostic is on unchanged source bytes, with the
five locations in changed files mapped to their exact base lines in
`build/advance-input-alias/clippy-attribution.json`. That command is not green.

The public oracle is:

```sh
for place in local field parameter call_result match_join const_name other_state other_machine target_body; do
  omega run --both samples/cli/facts/dynamic_float_refinement/input_alias/$place/main.omg
done
```

The focused exact-base comparison has **0 losses**: 48 source-check fixtures
(20 new checks/rejections and 28 original controls), and 21 runnable fixtures
(11 new positives and 10 original consumers). Original controls cover public
versus private guarantees, NaN/nonreflexivity, f32 rounding/format, negated
integer/IEEE/custom relations, changed captures, imported aliases, wrapping and
opaque selected operations. Scope and command outputs are retained in
`build/advance-input-alias/{source,corpus}-verdict.json`. The requested Jev
advisory was unavailable because no key is configured; it supplied no approval.

Only Linux x86-64 native execution was observed. Other hosted entries are
authored, not other-host native receipts. Governing rules are the
[dynamic slot public call shape](../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md),
[requirement/provider contract implications](../../../../../wiki/spec/language/machines.md),
[checked definition unfolding](../../../../../wiki/spec/proofs/mathematical_bindings.md),
and [IEEE comparison laws](../../../../../wiki/spec/language/numeric_values.md).

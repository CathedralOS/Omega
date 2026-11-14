# Open array const implementation

Parent: `STATIC-MACHINE-MIGRATION` in [TASKS.md](../../../../TASKS.md),
requirement "Complete const-expression closure ... outside the reference
producer's qualified scalar inline member path".

Fact: **Selected<[Base, Base + 4]> at Base = 3 supplies at least seven before exact value - 7.**

A fixed array is an eligible canonical const atom
([canonical static identities](../../../../wiki/spec/language/evaluation.md#canonical-static-identities)),
like integers, Booleans, records and cases. This row carries an open array
recipe through a selected named implementation's explicit argument tuple and
its element-projected contract (`Required[1]`), then consumes the published
lower bound with `value - 7`.

## Nearest row

The nearest row is
[`open_record_const_implementation`](../open_record_const_implementation/README.md)
(`Selected<Configuration { minimum: Base + 4 }>`). Its program has the same
shape, but its recipe is a nominal constructor and its contract projects a
declared field. An array literal has no nominal type of its own: it lands only
under the receiving binder's declared `[u64; 2]` carrier, and `Required[1]` is
an element position inside the declared extent. On the fixed base every place
of this row refused before checking reached the conformance, while the record
row passed. The three [controls](#controls) name obligations the record row's
twins do not reach: the element position must lie inside the declared extent,
the element count must equal it, and the whole array (not only the projected
element) is the implementation's identity.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [source](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const name | [source](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| target body | [source](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Each twin changes only the selected implementation's second element from
`Base + 4` to `Base + 3`; the required argument `[Base, Base + 4]`, the bound
and the consuming `value - 7` stay unchanged. The [root](main.omg) repeats the
local program. Every positive binds `ProgramEntry` for the four hosted targets
and for `macos_x86_64`, the Intel host where this row was measured, and
expects exit 0.

The [field place](field/main.omg) checks nonzero canaries on both sides of the
carried value. The [match join](match_join/main.omg) takes both Boolean arms
with distinct tags 17 and 29, so an inverted or wildcard dispatch traps. The
element values are 3 and 7: if lowering selected element 0 instead of element
1, `value - 7` would underflow and trap instead of exiting 0.

One additional production stays in this fact:
[private package projection](const_name/package_projection/main.omg) evaluates
the selected implementation through a dependency package whose member adds a
private record constant's `amount`; its
[twin](const_name/package_projection/invalid/main.omg) changes only the
package's own `Base + 4` element to `Base + 3`.

## Controls

- [element_outside_extent](controls/element_outside_extent/main.omg) projects
  `Required[2]` from a `[u64; 2]` binder in the trait's contract.
- [wrong_element_count](controls/wrong_element_count/main.omg) supplies a
  three-element recipe to the two-element implementation binder.
- [other_element_mismatch](controls/other_element_mismatch/main.omg) changes
  only element 0 (`Base + 1`); the projected element still matches, but the
  implementation's argument is a different array.
- [specialization_agreement](controls/specialization_agreement/main.omg)
  selects `[Base, 7]` for a bound that requires `[Base, Base + 4]`. The two
  agree only at Base = 3, the one specialization `main` uses, but the generic
  body must hold for every admitted Base, so it rejects in the body.

## Requirements

- **Establishment:** the selected `Selected` member's `result == Setting[1]`
  and the complete `Reading` application establish `result >= Required[1]`.
  `Base <= 100` establishes formation of each element recipe; no caller guard
  supplies the subtraction premise.
- **Identity and lifetime:** const binders are immutable and timeless; an
  element projection names the binder and a literal position, never runtime
  storage. Two arrays are the same argument only when every element agrees.
- **Composition:** simultaneous substitution replaces `Base` inside each
  element; calls, joins and state arrivals carry the same lower bound into
  `value - 7`.
- **Soundness boundaries:** element positions outside the declared extent,
  wrong element counts and mismatched elements reject; a projected element's
  equality does not stand in for the whole array's identity.
- **Psi 05–07:** specialization folds `Setting[1]` over the substituted literal
  to its element, so Terminal Psi carries ordinary `u64` arithmetic.
- **Omega 00–02:** the static array is erased; the result is an ordinary `u64`
  return. No new ABI placement lane.
- **Omega 03–06 and 07–09:** scalar selection, homes, frames and the image must
  produce exit 0 natively and agree with Terminal Psi.

## Evidence

Baseline: producer `gen-4f87c9e08b1` (revision
`4f87c9e08b1a7217960f7f69c440a9d2f09d278d`, opt-level 1, macOS x86_64). The
root, all nine places and the package production exited 200 under
`omega run --both`: "const argument `Required` has no static value with its
exact declared carrier". All ten twins and the three controls exited 1 with
that same diagnostic, which is not evidence that the fact is enforced. Sources
were frozen before the compiler was edited.

Repaired: an optimized (opt-level 1) producer built from this change on base
`11bdd2fe41ffb9faa4829046d0399795d90a655b`, macOS x86_64 host. Under
`omega run --both` the root, all nine places and the package production exit
0 natively and in Terminal Psi (11/11 agree). All ten twins and the
`other_element_mismatch` control reject with "generic machine `selected`
cannot bind `Implementation` to conformance `Selected`: expected a complete
`Reading` implementation with the instantiated trait arguments";
`element_outside_extent` rejects with "cannot form specification index in
trait `Reading` state `read` ensures contract: independent collection bounds
are not established by preceding formed facts"; `wrong_element_count` rejects
with "const array recipe has the wrong element count". Each `sample.json`
records those diagnostics.

The nine positives are pinned byte-identical under
`tests/omega/pass/facts/open_array_const_implementation_through_<place>_exit`,
the nine twins under
`tests/omega/fail/facts/open_array_const_implementation_through_<place>_rejected`,
and the three controls under
`tests/omega/fail/facts/open_array_const_implementation_<control>_rejected`.
Only the macOS x86_64 host was executed; the other four hosted targets keep
their authored bindings without a fresh execution claim.

The repair needed three changes beyond the array recipe support:

- A recipe rebuilt by specialization (`[3, 3 + 4]`) has no constructed type
  of its own, so its declaration construction judgment now takes the carrier
  its receiver declares and passes element and field carriers down.
- The prover's arithmetic terms and occurrence binders admit a literal
  element of an immutable Const array (`Required[1]`) exactly where they
  admit a Const record field; runtime indexed reads stay out.
- Constant initializers that call a bound requirement (`Implementation::read()`
  under `const VALUE: u64 = construct<3>()`) record the evidence binder's
  provenance span as the declaration source. A bound requirement child keeps
  only its spelling, so both source dependency collection and checked call
  custody refused it; the record row's pinned `const_name` fixture rejected on
  the same base for this reason and now checks again.

## Open

The row closes the fixed-array production of the parent's const-expression
closure and package projection. Case (tagged variant) recipes are not
exercised here, and `source/psi/` product-producer parity remains open.

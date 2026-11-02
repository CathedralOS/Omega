# Derived target-layout constants

Owning migration: **BUILD-OWNED-TARGET-SELECTION**. The underlying target-semantic
requirement remains in **PROVIDER-SELECTION-AFTER-TERMINAL**. The fact is: a target-selected
layout query contributes to a derived constant whose result discharges exact
subtraction and agrees with execution.

The original pre-migration primary program was frozen at SHA256
`4bb0e87fb321432dde92fbc1447989ccf9dad8d4b0b6c55cc99b872253dd5f36`.
It computes `const OBSERVED_SIZE: u64 = size_of<Main>() + 1u64`, transports that
value, and performs `observed - 5u64` before any equality guard. The runtime
branch traps unless both the exact value and subtraction result agree with the
target's authored expectation. Linux selects extent 4 and therefore value 5;
Windows/macOS select extent 8 and therefore value 9.

Historical implementation baseline main: `d6cf554fd1cbf20551983d29bae26f5cf3b81664`. Its optimized
CLI (opt-level 1, SHA256
`7eb4e799f088a9940752614a8f6b53a6dc43349f5e12d3030233cac0f520617e`)
runs the existing direct-query control with native and Terminal exits 0, but
rejects this unchanged primary in 0.341 seconds with CLI exit 200:
`constant initializer: constant call lost its exact authored target selection`
at line 22. The baseline build took 10m45s. Existing main already included
`6e37ba29d416499f1f717aa51452e13fa639853a`'s const-name provenance repair;
this advance uses it.

Acceptance: `omega run --both main.omg`, and the same command for each place.
All positive projects bind all four hosted `ProgramEntry` targets and promise
exit 0. The original retained qualification is Linux x86-64 native plus Terminal. Its
then-existing source-backed `target_constant_closure` example also ran a Windows
Terminal program and emitted a Windows image without executing that image. The
example has since been replaced; those observations retain their historical
producer identity. Explicit target checking/emission is not foreign-host execution.

| Place | Carrier of the same value | Pinned fixture suffix |
| --- | --- | --- |
| local | immutable local | `through_local_exit` |
| field | `SizeBox.observed` | `through_field_exit` |
| parameter | identity machine parameter/result | `through_parameter_exit` |
| call result | `read_size()` | `through_call_result_exit` |
| match join | both value arms | `through_match_join_exit` |
| const name | named derived constant | `through_const_name_exit` |
| other state | state arrival parameter | `through_other_state_exit` |
| other machine | attached machine result | `through_other_machine_exit` |
| target body | ordinary selected `DerivedObservation` result | `through_target_body_exit` |

The current source uses ordinary named implementations and explicit target-keyed
Build selections. `Answer` retains the exact Small/Large choices above.
`ExpectedExtent` publishes the complete `4 || 8` expectation, and the target-body
place selects `DerivedObservationCode`, whose original body still returns
`OBSERVED_SIZE`. Its public `5..9` interval justifies the caller's subtraction;
it does not claim general normalization of `result == OBSERVED_SIZE`.

The match place executes both original value paths, snapshots route 17 before
route 23 is written, and performs both exact subtractions before observing either
result. The value, geometry and subtraction comparisons remain separate runtime
falsifiers. Marker mutations must trap without changing the value contract.

The nine fixture pairs are
`tests/omega/{pass,fail}/facts/target_layout_derived_constant_through_*`.
Each `invalid/main.omg` changes only Small's and Large's provider result to 3.
The unchanged subtraction then rejects with target bounds `[-1, -1]` outside
`u64`. The root has the same reject twin. No caller equality guard establishes
the subtraction fact.

The repair follows the selected initializer dependency closure, rather than
recognizing a query only at its root. Target observations defer whole scalar
expressions and their aliases/helpers. After validated layouts close, the
ordinary scalar evaluator completes dependencies before consumers, preserves
source-call inventories, and consumes canonical values at exact selected const
uses. Call provenance does not choose the enclosing arithmetic operator.
Checked specializations close their own query subjects; their arena coordinates
are never read as identities in the separate authored forest. The previous
query-only leaf exemption and Omega's direct-query declaration stamping are
removed.

The specification and consuming layers are covered as follows:

- **Establishment:** selected validated geometry and admitted initializer
  arithmetic establish the bound; all nine reject twins reach the consuming
  exact subtraction.
- **Identity and lifetime:** exact declaration/selection identities survive
  aliases and copying. Source-backed receiving controls reject a different
  target, missing/repeated plan inputs, a copied digest and an equal-valued
  `Same` provider substituted for `Small`.
- **Composition:** `controls/` covers alignment composition, ordinary/generic
  helpers, a forward helper alias, public aliases and an authored `size_of`
  declaration. The helper underflow twins reject at `main`'s `slack` operation;
  their earlier range diagnostic is pinned explicitly.
- **Soundness boundaries:** an overflowing intermediate rejects even when later
  subtraction would cancel it. An unused carrier mismatch rejects. A skipped
  pure crashing call remains unexecuted; its demanded twin rejects for the
  unhandled Trap route. The former interval-qualified destination control now
  calls `bounded_layout` with the derived value and rejects its actual `value <= 3`
  obligation; the equivalent valid bound is 9. A cyclic target-constant graph rejects without provisional
  values. Same-product native `code_layout` remains forbidden. Existing
  declared-operator and unselected-provider controls remain rejecting.
- **Psi 06–08:** the same authored programs reach canonical Terminal Psi and
  execute their result/trap branch. The API example independently roundtrips
  canonical bytes and checks historical closure admission.
- **Omega 00–02:** historical plans retain exact provider and target provenance;
  actual checked query subjects use the selected layout authority. Ordinary
  scalar call/state placement carries the transported value.
- **Omega 03–06:** native instruction selection, register homes and frame
  realization execute the exact subtraction and result branch on each actually
  observed host.
- **Omega 07–09 and emission:** the emitted native image executes with exit 0 and agrees
  with Terminal. Retained run evidence includes the actual image, canonical
  Terminal artifact and engine streams; no compile-only result is counted.

The API example cited in the layer record is historical; its earlier canonical
roundtrip is not a current-source run.

Canonical authority is [semantic evaluation](../../../../wiki/spec/language/evaluation.md#target-dependent-applications),
[constant evaluation](../../../../wiki/spec/language/constants.md#evaluation),
[layout queries](../../../../wiki/spec/layouts/plans.md#size-and-alignment-queries)
and [Terminal compatibility](../../../../wiki/spec/terminal-psi/product.md).

The migration exposed two necessary general checking boundaries: a completed
return can carry its exact ordered observation even when replaying its source
expression at exit would be unsafe; the deferred range route still requires a
real target-dependent recipe. `controls/ground_return_guarantee` and its sole-value
4/5 twin keep an unused false ground guarantee from becoming a deferred proof.
The unused genuine target-false observation control also refuses.

Public target-dependent constants retain their authored semantic dependency
through package review rather than requiring a folded scalar. This description
retains exact query kind/operand and nominal alias links; it grants no geometry,
layout proof or execution authority. Those remain tied to checked source and
target closure. Changing a dependency changes public identity even when the
current scalar answers happen to coincide.

The migration is qualified with optimized producer `d6c89b306726`: all 38 authored
projects have the intended outcome, comprising 18 positive native/Terminal exit-0
agreements and 20 Source refusals. The unchanged `public_alias` now passes normal
package preparation, checking and both engines. All 36 scoped Source pins
(17 positive, 19 negative) and 17 native/Terminal pin pairs pass. Six existing
return-bound preservation controls have exactly the same status, facts and
diagnostics as the retained baseline. A positive Terminal counterfactual exits 0;
five sole mutations trap, distinguishing both match routes, observed geometry,
subtraction geometry and the selected provider's result. All four hosted profiles
pass explicit Source preparation/checking. Five metadata codec/identity tests and
one actual checked-custody mutation test also pass.

These fresh runtime observations are Mac ARM64 only, using that producer's
matching SDK. Four-target Source acceptance is not foreign-host execution. Older
partial `8316f371bf8` observations and the historical Linux qualification retain
their own producer identities.

The broader capability remains open. Late aggregate/qualified constants,
provider-free observation receipts, arithmetic layout predicates in ordinary
call preconditions, and package generic application review still need connected
implementation. A separate exploratory `requires value == 5/9` probe rejects
before closure; this row establishes scalar value/range transport, not arbitrary
predicate normalization. No language rule or authority gate is relaxed.

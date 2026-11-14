# Runtime function selection with an exact result

These programs choose a function at runtime, pass it the numbers 60 and 40,
and rely on its promise to return 100. Subtracting 100 must remain safe after
that choice passes through every place below.

Fact: a runtime dyn trait selector dispatches an inline conformance member over explicit data arguments to establish 100u64 when subtracting 100u64.

**BUILD-OWNED-TARGET-SELECTION** owns replacing the remaining platform-prefixed
functions with ordinary named implementations chosen in Build.
**RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** owns preserving the chosen
implementation, explicit arguments and public contract through both engines.
The input-arithmetic examples are another production of this same fact.
Neither parent task closes with this migration.

`WeightedCombine` requires exactly 60 and 40 and guarantees exactly 100.
`UnitCombine` and `OtherCombine` both meet that contract. To observe which
`combine` actually runs, each also receives an ordinary mutable reference to a
scalar owned by the caller. One writes its left argument there; the other writes
its right argument. The caller checks 60 and 40 in its two original scalars after
the calls, alongside both `total - 100` results. This detects a fixed or swapped
selection, reordered data arguments, a missing store, or a store to the wrong
place. A separate call returning a marker would not observe the actual
`combine` call.

The result promise justifies subtraction directly. The later runtime checks do
not establish a missing proof. The function choice (`dyn WeightedCombine`) and
all data, including the mutable output, stay explicit and independent under
[dynamic dispatch](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)
and [named implementations](../../../../wiki/spec/language/conformances.md#data-and-code-are-independent).

## Places and controls

| Place | Positive | Right-operand control | Uninitialized-selector control |
| --- | --- | --- | --- |
| local | [program](local/main.omg) | [reject](invalid_local/main.omg) | [reject](local/controls/uninitialized_selector/main.omg) |
| field | [program](field/main.omg) | [reject](invalid_field/main.omg) | [reject](field/controls/uninitialized_selector/main.omg) |
| parameter | [program](parameter/main.omg) | [reject](invalid_parameter/main.omg) | [reject](parameter/controls/uninitialized_selector/main.omg) |
| call_result | [program](call_result/main.omg) | [reject](invalid_call_result/main.omg) | [reject](call_result/controls/uninitialized_selector/main.omg) |
| match_join | [program](match_join/main.omg) | [reject](invalid_match_join/main.omg) | [reject](match_join/controls/uninitialized_selector/main.omg) |
| const_name | [program](const_name/main.omg) | [reject](invalid_const_name/main.omg) | [reject](const_name/controls/uninitialized_selector/main.omg) |
| other_state | [program](other_state/main.omg) | [reject](invalid_other_state/main.omg) | [reject](other_state/controls/uninitialized_selector/main.omg) |
| other_machine | [program](other_machine/main.omg) | [reject](invalid_other_machine/main.omg) | [reject](other_machine/controls/uninitialized_selector/main.omg) |
| target_body | [program](target_body/main.omg) | [reject](invalid_target_body/main.omg) | [reject](target_body/controls/uninitialized_selector/main.omg) |

The [root](main.omg) uses the local production. Every positive binds all four
required hosted targets and expects exit 0. The target place also retains the
macOS x86-64 binding added by the concurrent migration. Both match branches run with distinct choices.
The field place also checks the scalar neighbors 17 and 29 around its selector.
The `other_state` path retains the nested-machine place.

Each ordinary reject twin changes only the transported right operand and its
bounds from 40 to 39. The public `WeightedCombine` requirement still demands
40, so rejection must identify its unproved call precondition. Each additional
uninitialized-selector control removes selector establishment. The match control
keeps both arms and reads an uninitialized `vacant` value in each. The field
control initializes both scalar neighbors while leaving only its selector unset.
These controls must reject at the ordinary validity check before an unestablished choice can be
used. Neither control substitutes a different caller guard.

The [input-arithmetic production](input_arithmetic/README.md) keeps all nine
positives and twins. Its caller proves the same exact left-operand requirement
from an independently representable bounded addition. Its existing distinct
markers, both match choices and field neighbors remain unchanged. The complete
selection is 46 sample projects and 45 corpus fixtures, including every control.
Sample Main and Build files match their pins under
`tests/omega/{pass,fail}/facts/dynamic_trait_dispatch_through_*`,
`dynamic_trait_input_arithmetic_through_*`, and
`uninitialized_dyn_selection_through_*`.

## Selection by Build

The target place calls the ordinary `SelectedCombine::probe` requirement. It
forwards the caller's function choice, scalar arguments and output reference;
the inner `WeightedCombine::combine` call then uses that choice.
The four original target bodies performed the same computation, so five explicit
Build rows select one ordinary `HostedCombine` implementation. Each entry calls
that same requirement. The two runtime function choices remain independently
observable through their direct writes. Application identities remain unchanged.

This follows [Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation).
The public requirement declares no crashes or service use. The entry records its
direct synchronous call with `invokes SelectedCombine`, preserving that empty
service promise under [service reach and direct invocation](../../../../wiki/spec/language/effects.md).
All implementations are checked independently of the host selected for execution.

## Requirements established

- **Establishment:** checked members prove the exact result under the public input
  bounds. A selector is established by a named implementation or an accepted copy;
  zeroed storage does not establish one.
- **Identity and lifetime:** every place preserves its actual function choice and
  scalar arguments. The output is an explicit mutable reference to the caller's
  storage, whose value is read after the call completes. Choosing code grants no
  hidden receiver, data ownership or loan.
- **Composition:** calls substitute the actual inputs, joins keep the actual
  choice from each branch, and nested calls preserve their own arrival obligations.
  The result guarantee directly proves subtraction. The operand twins and
  uninitialized controls challenge those separate obligations.
- **Soundness:** public contracts remain mandatory independently of the selected
  member. No stronger private promise or post-call equality guard proves an
  otherwise invalid call.
- **Terminal Psi:** the program retains the selector separately from data
  arguments, the indirect call, its scalar result and the store through the
  caller's mutable reference. Both engines must observe that original storage.
- **Providers, layout and calling plans:** explicit Build rows choose the target
  implementations. The selector is its own scalar word; scalar data and output
  references retain their ordinary layouts and argument positions.
- **Instructions and activation storage:** indirect calls must deliver the right
  arguments and write the original output home. Selector fields and their checked
  neighbors must use their declared offsets.
- **Encoding and execution:** dispatch-table references and emitted calls must
  select the authored implementation. Source acceptance or successful image
  emission alone does not demonstrate the runtime requirement.

## Validation and remaining scope

The reconciled source `281c0d530cb7a5c108e8301ea6b5851f441244e1` passes the
complete focused scope on [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41565512619)
and [macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41565515991).
On each host, eighteen positive fixtures pass checking and exit 0 in both native
and Terminal execution; twenty-seven controls reject for their intended reasons.
All forty-six sample projects meet their oracles: nineteen executions and
twenty-seven intended rejections. Both producers have verified optimization level 1.

This replay follows an actual conflict with main
`6b64ebbaa6bafedc9822013c3965a271a5f173b9`. Its incoming compiler changes affect
range proofs, storage and field checking, lowering and both execution engines,
so all cases were rerun. All Main programs and expected outcomes remain identical
to the accepted pre-merge candidate `4ddfb1d9339e902e04c0cb88fc145204b6aaeeba`.
Six matched target Build files also retain main's optional macOS x86-64 entry.
The audit verified eighteen downloaded report hashes, all 320 authored input
hashes and ninety matching Main/Build sample-pin pairs.

The [published source archive](https://github.com/CathedralOS/Omega/pull/1567#issuecomment-6594136272)
retains exactly the source measured by this replay. Records name that source;
later documentation and measured-record updates do not claim another execution.
Only twenty-seven owned checking records and nine owned combined-execution
records per host are replaced. Every unrelated block from incoming main is
unchanged. Compared with main, the two target-body errors become one shared-helper
error in each target control. The nested step's verified result guarantee removes
one secondary outer-guarantee error. Each primary rejection cause remains, and
the match control still reports both vacant-arm reads.

The earlier accepted observations remain historical:

- Source `0a53590a2991a39c810225fade12860d577192df` supplied forty unchanged
  fixture checks, seventeen positive fixture executions and forty-one unchanged
  sample results, from the full [Linux](https://github.com/CathedralOS/Omega/actions/runs/41550121602)
  and [macOS](https://github.com/CathedralOS/Omega/actions/runs/41550126485) runs.
- Source `b9b495ed6d3d109a2620cf35dbfbf5deafbce91d` supplied the then-current target
  positive, operand twin and uninitialized-selector control: three checks, one
  positive fixture execution and three sample results on
  [Linux](https://github.com/CathedralOS/Omega/actions/runs/41552534492) and
  [macOS](https://github.com/CathedralOS/Omega/actions/runs/41552537806).
- Source `a34a0eae56fa35097331cb4368d1b85cd9362e0f` supplied the restored match
  control and the field control with initialized scalar neighbors, each checked
  as a fixture and an ordinary intended-invalid project on
  [Linux](https://github.com/CathedralOS/Omega/actions/runs/41559598582) and
  [macOS](https://github.com/CathedralOS/Omega/actions/runs/41559602325).

For those earlier observations, each host used the same verified, optimized
compiler binary across its three runs. The earlier reuse review covered unchanged
self-contained projects and unchanged compiler, library, policy and gate inputs.
Those six runs are not presented as execution of the integrated compiler.

Every operand control names its unproved public call requirement. The field
control refuses when reading `dispatcher.selector` after both scalar stores;
the match control reports both vacant-arm reads. The historical ledger records
the earlier four-to-one target-body reductions and the removed secondary
outer-guarantee error. Those observations remain
retained alongside the new comparison against incoming main.

The unchanged [Linux baseline](https://github.com/CathedralOS/Omega/actions/runs/41547276197)
at documentation-only checkpoint `1cc1fb70bc837a91b4adbf4061d4869695460d4d`
also covers all original forty-five fixtures and forty-six sample projects.
Its compiler, library and authored program inputs match warm-chain integration
`15dfb9e3dff868befb2ddec587b0ed51d5accbf8`, whose tree matches published predecessor
`4f3360c7e2e3869002c325fc1157780209e9a599`. Accepted platform prefixes were
migration inputs, rather than a newly demonstrated compiler defect.

Source archives are published with the PR evidence; raw reports, profiles and
input hashes remain under `build/dynamic-trait-selected-20261102/` and in the
linked hosted artifacts. Windows x86-64 and Linux ARM64 execution remain
unverified. No compiler repair was required for this migration.

Earlier documentation reports macOS ARM64 observations at `2d783d092d9d` and
fixture-name/record updates at `114906953e1`, with runner SHA-256
`f14e17c7521b980e193ba6929baec279dd843ec8144214f42e19e4e94cacfe5a`.
Those records describe their original programs. The earlier uninitialized-selector
repair reports that producer `2214d8` accepted invalid storage and a direct
projection reached native signal 11 and Terminal `VerifiedOperationMalformed`;
its checked-establishment repair and existing array/store controls remain retained.
None of those observations validates the changed programs here.

The previous README also described a body returning 101 under the exact-100
contract as a runtime selector test. Such a body cannot be a valid implementation
of that contract, and no retained run here establishes the claimed observation.
That description supplies no qualification evidence. The current direct stores
allow both implementations to remain valid while their actual calls are observable.

Broader dynamic operational contracts, generic method families, cross-package
visibility, product-compiler parity, and the remaining target-prefixed source
families remain parent acceptance. Other-host execution is unverified until
recorded. No owner design decision was required for this migration.

The concurrent target migration in PR #1575 reports an earlier `11bdd2fe41f`
macOS x86-64 measurement: its then-current target positive exited 0 in both
engines, and its operand and uninitialized-selector controls rejected. It used
separate tag-check wrappers and a different target requirement. Those observations
remain historical. The optional macOS x86-64 Build key is retained here through
the shared provider; execution of these stronger current programs on that host
remains unverified.

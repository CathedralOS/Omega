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
| local | [program](local/main.omg) | [reject](local/invalid/main.omg) | [reject](local/controls/uninitialized_selector/main.omg) |
| field | [program](field/main.omg) | [reject](field/invalid/main.omg) | [reject](field/controls/uninitialized_selector/main.omg) |
| parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) | [reject](parameter/controls/uninitialized_selector/main.omg) |
| call_result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) | [reject](call_result/controls/uninitialized_selector/main.omg) |
| match_join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) | [reject](match_join/controls/uninitialized_selector/main.omg) |
| const_name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) | [reject](const_name/controls/uninitialized_selector/main.omg) |
| other_state | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) | [reject](other_state/controls/uninitialized_selector/main.omg) |
| other_machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) | [reject](other_machine/controls/uninitialized_selector/main.omg) |
| target_body | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) | [reject](target_body/controls/uninitialized_selector/main.omg) |

The [root](main.omg) uses the local production. Every positive binds all four
hosted targets and expects exit 0. Both match branches run with distinct choices.
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
The four original target bodies performed the same computation, so four explicit
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

The unchanged [Linux baseline](https://github.com/CathedralOS/Omega/actions/runs/41547276197)
checks eighteen positives and rejects twenty-seven intended controls. All eighteen
fixture executions exit 0 in both engines; all forty-six sample projects meet
their oracles (nineteen executions and twenty-seven intended rejections).
The producer uses verified optimization level 1. Migration observations remain
pending. The fixed warm-chain
integration is `15dfb9e3dff868befb2ddec587b0ed51d5accbf8`, whose tree matches the
published predecessor `4f3360c7e2e3869002c325fc1157780209e9a599`. Documentation-only
checkpoint `1cc1fb70bc837a91b4adbf4061d4869695460d4d` retains all original compiler,
library and authored program inputs. Existing accepted platform prefixes are
migration inputs, rather than a newly demonstrated compiler defect.

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

# Target-keyed provider results

A target-keyed provider call establishes `total >= 1000`, so exact subtraction
`total - 1000` preserves the selected provider result.

Owner: [BUILD-OWNED-TARGET-SELECTION](../../../../TASKS.md). This row establishes
that evaluated target keys choose ordinary named implementations while the
portable boundary call retains only its public contract. It is the executable
requirement for the explicit Build provider-row protocol; it does not close the
remaining target-family retirement or native-binding migration.

The governing contracts are [provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection),
[multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation),
and [proof contracts](../../../../wiki/spec/proofs/contracts.md).

| Place | Positive | Public-contract control |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

`FirstReading` returns 1100 and `SecondReading` returns 2100. The same portable
`Reading::total()` call has explicit Build rows selecting the first implementation
for Windows and Linux x86-64, and the second for macOS and Linux ARM64. Both
implementations are checked, and their stronger private result values do not
become a caller premise. Each reject twin weakens only the public lower bound
to 999; the implementation bodies and their stronger contracts stay unchanged.
Exact subtraction or its forwarding contract must then refuse.

Each target entry checks the corresponding residual (100 or 1100) and traps if
provider selection changes. The field place checks neighboring canaries. The
match join runs both arms with different tags, so inverting the subject or
choosing the fallback cannot pass. The selected-target-body place calls an
ordinary named implementation; none of this row uses a source target prefix.
Every positive binds all four hosted ProgramEntry slots and expects exit 0.

## Requirements and evidence

- **Establishment:** `Reading::total` promises the lower bound; every provider
  independently proves it. No caller equality guard supplies the arithmetic premise.
- **Identity and lifetime:** the bound follows the exact returned scalar through
  each transport place. Each Build row retains exact requirement, implementation,
  authored occurrence and evaluated catalog target under the original Build activation.
- **Composition:** forwarding and arrival contracts preserve the bound, match
  joins preserve only the public fact, and subtraction consumes that bound directly.
- **Soundness:** the controls must refuse even though each selected implementation
  happens to return a sufficient number. Selection grants no access or trust authority.
- **Psi 05–07:** ordinary boundary calls and checked provider bodies lower to one
  target-neutral Terminal program. The Build companion retains target-keyed selection
  separately from source checking. Actual `run --both` observations require agreement.
- **Omega 00–02:** whole-provider selection filters exact deployment profiles before
  duplicate/default arbitration and publishes through the existing provider companion.
  Catalog target values are ordinary checked data, not a selected-host observation.
- **Omega 03–06:** scalar calls, return homes, subtraction and result comparisons
  pass through ordinary ABI, instruction, register and activation-frame realization.
- **Omega 07–09:** the authored result is observed after native image emission and
  compared with Terminal interpretation; a wrong provider has a different result.

The row is pinned under `tests/omega/{pass,fail}/facts/build_target_provider_through_*`.
Producer-bound observations live in the qualification record and commit. Fresh
local observations are macOS ARM64 only; no Windows/Linux native execution is
claimed from cross-compilation or source checks.

Remaining parent acceptance: retire source prefixes and all family IR/codec/runtime
paths; migrate their callers, layouts, callbacks, entry and audio witnesses;
complete static/lifetime applications, selected-closure refusal controls and
matching-host observations for the other targets. Build-owned native locators
are the paired BUILD-OWNED-NATIVE-BINDINGS outcome and remain separate acceptance.

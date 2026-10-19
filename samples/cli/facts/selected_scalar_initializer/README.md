# Selected scalar initialization

Owning requirement: **TERMINAL-SLICE-VIEW-VOCABULARY**, specifically construction of nonempty restricted scalar arrays and primitive locals when zero does not satisfy their declared predicate. This row establishes a closed non-interval initializer predicate through the existing source-to-Terminal-to-native path.

An explicitly initialized `[i32 in Selected; 2]` retains the nonzero disjunctive domain **1 or 38**, and indexing its second element observes **38**.

The original [root](main.omg) is frozen unchanged. All positives bind the four hosted ProgramEntry targets and declare exit 0. Each place runs with both Boolean inputs; the match join executes both incoming array arms.

| Positive place | One-obligation twin |
|---|---|
| [local](local/main.omg) | [reject](local/invalid/main.omg) |
| [field](field/main.omg) | [reject](field/invalid/main.omg) |
| [parameter](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| [call_result](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| [match_join](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| [const_name](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| [other_state](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| [other_machine](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| [target_body](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

Every twin substitutes one authored `[1, 38]` initializer with `[1, 2]`. The match twin changes only the false arm; the target-body twin changes only Windows. Array length 2 and index 1 remain unchanged. Const formation rejects the false const-leaf domain; other twins reject array-element domain establishment. Derived call/arrival refusals can repeat that one missing membership. A missing import or unrelated lowering refusal does not count.

## Establishment and identity

`Selected(value)` is predicate-only: `value == 1 || value == 38`. Source construction establishes membership separately for each literal leaf before any readable value exists. Zero belongs to neither alternative. The second element's exact value 38 comes from its explicit initializer, not from treating disjunction as equality. The qualified indexed read consumes the retained membership; the later runtime comparison checks identity/transport and never supplies the static fact.

Local and field constructors retain their exact receiving types. Parameters and internal states transport the established array, the call-result machine constructs it, and the relay machine forwards its owned value. Const leaves establish membership during constant checking. The match join requires both incoming qualified values. Target selection narrows realization, while Psi checks every target body.

Mutation must establish the receiving predicate again. The focused nonzero scalar snapshot variation copies 38 before replacing the current value with 1; the snapshot remains 38. Existing missing-domain, invalidated-domain, partial-coverage and shared-view-write controls preserve initialization, lifetime and access boundaries. Predicate truth supplies no routed-domain authority.

## Carrier and execution

The common primitive initialization operation retains an obligation for the complete receiving restriction over the completed SSA operand. It uses the existing scalar receiving-predicate owner for both intervals and closed refinements. The Terminal operation retains its exact value, declaration and obligation; optimization admission preserves machine/operation/obligation/current-Psi identity and the full receiving predicate. Nothing grants membership merely from a type label or from zeroed storage.

`i32 in Selected` has the ordinary i32 carrier and `[i32 in Selected; 2]` has two such elements. There is no runtime predicate tag, wrapper, hidden storage, changed array length or qualification-specific ABI. Existing primitive stores, structural array construction, call/result homes, state transfers and image emission realize those same carriers. The interpreter observes the same completed operands as native execution.

The available executor is Linux x86-64. Acceptance records actual `omega run --both` native and Terminal exits on that matching host. Windows x86-64, macOS ARM64 and Linux ARM64 bindings are authored and source-checked; native execution on those unavailable hosts is not claimed.

## Before and after

At fetched baseline `97fca7a18f744b0f7c0b54811b7d41b586d6bfb7`, the root and all nine positives source-check. Every positive then refuses native production with exit 200, before either engine runs: **primitive storage has no supported initialization restriction**. The unchanged smoke fixture executes native 0 / Terminal 0. Two baseline setup attempts overlapped manifest finalization; their refusals and successful corrected preparations are retained separately.

The repair removes the interval-only initialization recognizer and reuses `scalar_field_requires_evidence`. Primitive storage recognition also admits the already-retained closed scalar predicate; explicit initializers and incoming values still establish storage, and every replacement retains its complete receiving restriction. An invalid replacement of 38 with 2 rejects at the `Selected` write obligation. Ordinary source establishment and all existing receiving/custody consumers remain authoritative. Final focused acceptance observes 14 native/Terminal cases at exit 0/0 (nine places, the snapshot, and four existing controls), plus 14 source accepts and 17 intended rejections with no wrong-reason twins. Explicit four-target source checks cover every place. Final command/revision/profile, source rejection and engine observations are retained in the qualification and claim evidence.

Canonical authority: [domains](../../../../wiki/spec/language/domains.md), blob `910129f88502e7c60625aed8346be9d547c3b41b`, and [default domains and zero initialization](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization), blob `357c121a4b972f3dd1d2ab8307ba0b8ce9a47a0b`. Their exact excerpts and revision fingerprints are included in implementation and independent-review context.

The existing `runtime_predicate_qualified_scalar_copy_exit` control returns native 0 but Terminal declines its borrowed-view argument preparation. The preserved baseline compiler reproduces the same refusal; this remains an unrelated parent gap, not passing engine agreement.

Remaining parent acceptance: non-interval refined scalar parameter/state-arrival contracts, narrowed record-element view default-domain restoration, cyclic Unit-graph view forwarding, and the parent's remaining consuming scalar/record view combinations. This bounded row does not close the parent capability or the full multi-host release matrix.

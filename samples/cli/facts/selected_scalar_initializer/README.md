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

The available executor for this recording was macOS ARM64. Acceptance records actual `omega run --both` native and Terminal exits on that host; every place binds all four hosted `ProgramEntry` targets and all four target bodies are source-checked, while native execution on the unavailable Windows x86-64 and Linux targets is not claimed. A prior revision of this section named Linux x86-64 as the executor, which was the host of the earlier linux golden rows; the live recording host is macOS ARM64 and the rows below are stamped with it.

## Before and after

At fetched baseline `97fca7a18f744b0f7c0b54811b7d41b586d6bfb7`, the root and all nine positives source-check. Every positive then refuses native production with exit 200, before either engine runs: **primitive storage has no supported initialization restriction**. The unchanged smoke fixture executes native 0 / Terminal 0. Two baseline setup attempts overlapped manifest finalization; their refusals and successful corrected preparations are retained separately.

The repair removes the interval-only initialization recognizer and reuses `scalar_field_requires_evidence`. Primitive storage recognition also admits the already-retained closed scalar predicate; explicit initializers and incoming values still establish storage, and every replacement retains its complete receiving restriction. An invalid replacement of 38 with 2 rejects at the `Selected` write obligation. Ordinary source establishment and all existing receiving/custody consumers remain authoritative. Final focused acceptance observes 14 native/Terminal cases at exit 0/0 (nine places, the snapshot, and four existing controls), plus 14 source accepts and 17 intended rejections with no wrong-reason twins. Explicit four-target source checks cover every place. Final command/revision/profile, source rejection and engine observations are retained in the qualification and claim evidence.

Canonical authority: [domains](../../../../wiki/spec/language/domains.md), blob `910129f88502e7c60625aed8346be9d547c3b41b`, and [default domains and zero initialization](../../../../wiki/spec/language/dependent_values.md#default-domains-and-zero-initialization), blob `357c121a4b972f3dd1d2ab8307ba0b8ce9a47a0b`. Their exact excerpts and revision fingerprints are included in implementation and independent-review context.

The existing `runtime_predicate_qualified_scalar_copy_exit` control returns native 0 but Terminal declines its borrowed-view argument preparation. The preserved baseline compiler reproduces the same refusal; this remains an unrelated parent gap, not passing engine agreement.

Remaining parent acceptance: non-interval refined scalar parameter/state-arrival contracts, narrowed record-element view default-domain restoration, cyclic Unit-graph view forwarding, and the parent's remaining consuming scalar/record view combinations. This bounded row does not close the parent capability or the full multi-host release matrix.

## Requirements and evidence

Owning task: **TERMINAL-SLICE-VIEW-VOCABULARY**, requirement *construction of nonempty restricted scalar arrays and primitive locals when zero does not satisfy their declared predicate*. Every positive place and the root bind `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64` and `linux_arm64`, declare `Expected exit: 0`, and run with both Boolean inputs so the match join executes both incoming array arms.

- **Establishment.** Each literal leaf establishes its own membership in `1 || 38` before any readable value exists; zero belongs to neither alternative. The consuming read of `items[1]` and the comparison `observed == 38` are proved from the retained membership, never re-established by a post-hoc caller equality guard.
- **Identity and lifetime.** The second element's exact value 38 comes from its explicit initializer, not from treating the disjunction as equality. Replacement must re-establish the receiving predicate: the snapshot variation copies 38 before overwriting with 1, and the snapshot keeps 38. Invalidation of unrelated storage does not discard the fact.
- **Composition.** Local and field constructors keep their exact receiving types; parameters and successor states transport the established array; the call-result machine constructs it; the relay machine forwards its owned value; const leaves establish membership during constant checking; the match join requires both incoming qualified values. `git diff` confirms each `<place>/main.omg` differs from its `<place>/invalid/main.omg` only in the authored initializer `[1, 38]` vs `[1, 2]`, plus the match twin's false arm and the target-body twin's Windows body — never in a caller guard.
- **Soundness boundaries.** Predicate truth supplies no routed-domain authority. Ownership and access authority are preserved; the focused controls for missing domain, invalidated domain, partial coverage and shared-view write keep their boundaries.
- **Terminal Psi and carrier lowering.** The common primitive initialization operation retains an obligation for the complete receiving restriction over the completed SSA operand, using the scalar receiving-predicate owner for both intervals and closed refinements. No runtime predicate tag, wrapper, hidden storage or qualification-specific ABI exists: `i32 in Selected` has the ordinary i32 carrier and the array has two such elements.
- **Native realization and execution agreement.** Existing primitive stores, structural array construction, call/result homes, state transfers and image emission realize those carriers. Measured on the recording host: nine pass fixtures `built ... exit:0` in `tests/omega/corpus_native_macos_arm64.txt`, and nine `built ... exit:0 interpreter-exit:0 interpreter:agrees` rows in `tests/omega/corpus_both_macos_arm64.txt`.

The row is pinned by `tests/omega/{pass,fail}/facts/selected_scalar_initializer_through_*`; the check leg reports 18 fixtures as 9 `checked` and 9 `rejected` with no reject missing its expected fragment and no fail fixture that failed to reject. Nine sample-side positives meet their documented oracle on `macos_arm64` and nine intended-invalid controls match their authored rejection.

Discrimination is measured, not assumed. Corrupting the construct's own input to `[1, 2]` is refused during checking with `array literal element is not proven in domain `i32::Selected``. Corrupting the *selected result* instead — reading `items[0]` while the initializer stays valid — is still accepted by checking and then traps in **both** engines: native reports `native signal 5` and Terminal reports `interp crash: Trap (host presentation agrees)`. The recorded `exit:0` rows therefore measure the selected value, not merely a successful build.

## Run records

| Check | Command | Result |
| --- | --- | --- |
| native golden | `python3 tools/corpus_gate.py --native --record --filter facts/selected_scalar_initializer` | 9 rows `built ... exit:0` |
| engine agreement | `python3 tools/corpus_gate.py --both --record --filter facts/selected_scalar_initializer` | 9 rows `interpreter-exit:0 interpreter:agrees` |
| check leg | `python3 tools/corpus_gate.py --filter facts/selected_scalar_initializer` | 18 fixtures: 9 `checked`, 9 `rejected`, 0 structural moves |
| sample gate | `python3 tools/sample_native_gate.py --filter cli/facts/selected_scalar_initializer --facts` | 10/10 meet the oracle; 9/9 controls match |

Producer: the checkout's `omega` at opt-level 1 (`omega --build-profile` reports `opt_level: 1`), revision `f98259e86dd2` for the native rows and `67b7c6dfba3` for the agreement rows, host macOS ARM64.

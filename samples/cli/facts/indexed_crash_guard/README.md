# Indexed entry crash guard

Fact: the exact selected element `items[index] == 7` discharges the callee's indexed zero crash guard and returns that same 7 without a caller crash ceiling. The independent bound is `index < 2`; the consuming operation is `maybe_zero(items, index)`.

This row advances **CRASH-CONTRACT**: entry-operand normalization and invocation-specific disproof for dynamic indexed integer observations. It does not close that outcome's other generic, structural, provider, or crash-execution obligations.

The original [root](main.omg) is preserved byte for byte. Its private [control](invalid/main.omg) legitimately requests inference after deleting the equality: both check, because private omission permits inferred Trap and main already permits Trap. It is not counted as a reject twin. The nine probes expose the existing public omitted-cause prohibition on `read_seven`; none adds a `crashes` clause there. Every pair differs only by deleting its selected-element equality requirement.

| Place | Positive | Reject twin | Transfer |
| --- | --- | --- | --- |
| Local | [source](local/main.omg) | [control](local/invalid/main.omg) | Immutable selector copy |
| Field | [source](field/main.omg) | [control](field/invalid/main.omg) | Exact record selector field |
| Parameter | [source](parameter/main.omg) | [control](parameter/invalid/main.omg) | Original entry selector |
| Call result | [source](call_result/main.omg) | [control](call_result/invalid/main.omg) | Checked normal `result == index` guarantee |
| Match join | [source](match_join/main.omg) | [control](match_join/invalid/main.omg) | Both arms deliver the same selector; both execute |
| Const name | [source](const_name/main.omg) | [control](const_name/invalid/main.omg) | Named seven retains exact value |
| Another state | [source](other_state/main.omg) | [control](other_state/invalid/main.omg) | Renamed collection and selector arrival |
| Another machine | [source](other_machine/main.omg) | [control](other_machine/invalid/main.omg) | Forwarded same zero-guarded route |
| Another target body | [source](target_body/main.omg) | [control](target_body/invalid/main.omg) | Target-family forwarder preserves the route |

The corpus pairs are `tests/omega/{pass,fail}/facts/indexed_crash_guard_through_<place>`. Public twins must refuse the uncovered Trap at `read_seven`'s call to `maybe_zero` or `forward`, rather than an unrelated formation error.

Establishment comes from the entry equality and independently checked bounds. Identity joins the exact collection, selector and invocation version, including dense scalar positions separated from authored structural positions. Composition retains checked normal guarantees, all-arm joins and all-arrival state provenance. Writes and unknown origins cannot establish a substituted entry operand. Saved Boolean guards supplement the exact incoming conjunction with authenticated consequences; they never replace it.

Source checking, Terminal Psi lowering and native realization consume the existing indexed-read representation and each indexing occurrence's bounds evidence. All positives bind all four hosted ProgramEntry targets and declare exit zero. Execution evidence is Linux x86-64 `omega run --both`; other target bindings are source/publication coverage, not native execution claims for unavailable hosts. Qualification and claim receipts retain exact producers, commands, outcomes and timings.

Canonical authority: [crash call composition](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md#crash), [guarded crashes and public omission](../../../../wiki/spec/language/effects.md#guarded-crashes), and [independent numeric formation](../../../../wiki/spec/language/numeric_values.md). Broader crash-frontier, custody, selected-provider and other-host execution obligations remain governed by those contracts.

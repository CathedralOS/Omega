# Finite choice cycles

Choosing one of two mutable inputs and writing through that result preserves a disjoint index-bound fact on the other input.

This is another production of the [R5 helper-choice fact](../README.md). `choose`
passes the selected reference first to `rotate`. One arrival swaps the two
references and clears `again`; the next arrival returns the second reference.
The normal result therefore names the selected original input. `overwrite`
writes `9` through that result while evaluating `add`'s operand. Indexing the
unchanged input directly consumes its previously established bound. Every
reject twin changes only `choose`'s selector from `true` to `false`, so the
write overlaps the index and its bound must be retired.

| Place | Positive | Overlap twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) |
| another target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) |

At baseline `036605466510d2361fd579cf9fd97b722e070beb`, all nine positives
wrongly reject with `cannot prove index ... is within length 4`, using a freshly
built optimized compiler. The reproduction command is:

```sh
omega run --both samples/cli/facts/may_write_choice/finite_cycle/local/main.omg
```

The governing contracts are [invocation contracts](../../../../../wiki/spec/language/state_contracts.md)
and [finite subject permutations](../../../../../wiki/spec/language/termination.md#subject-preservation).
The original row's eight requirement layers apply unchanged: construction or
entry contracts establish the bound; identity is the original referenced
storage; simultaneous arrivals compose the captured reference permutation and
Boolean snapshots; overlap still invalidates the bound without granting loans
or custody. The same source lowers through Terminal Psi, provider/ABI planning,
instruction selection, frame realization and native emission. Every positive
binds all four hosted targets, with exit `0` required in both engines on a
matching host. Pinned fixtures are
`tests/omega/{pass,fail}/facts/may_write_choice_finite_cycle_through_<place>_*`.

The [same-query control](same_query/main.omg) uses a decreasing non-Boolean
counter, so successive arrivals have the same three-valued query key. Keeping
reference roots in place returns only the first input and exits `0` in both
engines. Its [twin](same_query/invalid/main.omg) changes only the backedge's
reference permutation; its possible return roots include the indexed input and
the stale bound rejects. The [unknown-selector control](unknown_selector/invalid/main.omg)
also rejects: finite convergence must not invent a selector value.

## Acceptance and remaining dependency

On Linux x86-64, all nine finite-cycle positives exit `0` in Terminal Psi and
native execution; all nine overlap twins reject their stale index bounds. The
same-query positive also agrees at `0`, and its swap twin and the unknown
selector reject. All seven older may-write rows (including stored codec
references) retain their 63 dual-engine positives and 63 overlap rejections.
Existing root programs and the written-selector witness also retain exit `0`.
The affected `validation` crate check passes. Focused corpus records pin ten
positives and eleven rejecting controls; no unrelated golden rows are changed.

The parser check converges: baseline `395.85 s`, repaired `370.76 s`. The
zero-argument harness wrapper fixes the baseline arrival-parameter diagnostics.
A successful check from accepted package inputs remains blocked by normal
standard-library admission: `omega update --project tests/psi/parser --target
linux_x86_64` rejects ten `FloatMeaning` contract entailments. The preserved
baseline compiler produces the exact same ten diagnostics. R5 remains open for
that dependency; these finite-cycle observations do not claim package acceptance.
Other hosted targets are bound and source-checked; foreign native execution was
not available on this Linux executor.

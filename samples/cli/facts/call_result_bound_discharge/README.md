# call_result_bound_discharge

A callee's `requires` discharges on a bound the caller learned from
another call's closed result: the result is 4, so `echo` proves
`scale <= 5` structurally at the instantiated judgment.

That one sentence is the fact. Only the transport of the closed result
changes across the nine places; the bound (`scale <= 5`), the operation
(`echo`), and the observed value (4) stay fixed. A place that changed any of
them would be a different fact and would not count.

This fact is one row of nine places. Each place has one accepting project
that binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64` and `linux_arm64`, carries `expected_native_exit.txt` = 0 and
the header `// Expected exit: 0`, traps on a wrong result (`crash Trap;` in
`state fail`), and agrees across native and Terminal execution. Each place
has one reject twin fed a closed result of 9 instead of 4, which rejects at
the same `scale <= 5` obligation.

| Place | Project | Transport of the closed result |
| --- | --- | --- |
| local | `local/` | bound to a local before the call |
| field | `field/` | stored in a record field |
| parameter | `parameter/` | arrives as an explicit parameter |
| call result | `call_result/` | passed as the call result directly |
| match join | `match_join/` | both match arms yield it |
| const name | `const_name/` | named constant `SCALE` |
| another state | `other_state/` | a successor state receives it |
| another machine | `other_machine/` | a forwarding machine returns it |
| another target body | `target_body/` | each of four target bodies uses it |

## Requirement families and owning task

- Parent outcome: `ONE-INTEGER-BOUNDS-ENGINE` (`TASKS.md`)
- Requirement families: `language-state-contracts-001`,
  `terminal-psi-fact-validity-001`, `terminal-psi-integer-certificates-001`

## What this row discharges, and what stays open

**Establishment.** `opaque()` is a closed machine whose body is the literal
`4u64`, so its result is exact at the call site. The caller does not guard
the bound afterwards; `echo`'s own `requires scale <= 5` is discharged by the
instantiated judgment on the argument the call supplies. Removing the
closed result removes the proof, which is what the twins demonstrate.

**Identity and lifetime.** Every place names one exact value: the result of
one call to `opaque()`. No place re-establishes the fact from a later
comparison or from a caller guard, and the twins differ from their positives
only in that same input value (4 versus 9) — verified by `diff` of
`<place>/main.omg` against `<place>/controls/invalid/main.omg`. The
`other_machine` twin additionally drops the forwarding `ensures result <= 5`
rather than the input, which is the construct's own contract and is the
permitted difference.

**Composition.** The consuming operation is the call `echo(s)`, and its
required fact is proved directly from the value carried through the place.
The `parameter`, `other_state` and `target_body` places carry the fact across
an intermediate machine, state contract and target-scoped body respectively,
so the substitution is exercised across more than a straight line.

**Soundness boundaries.** All nine twins reject rather than execute, so the
fact is enforced at checking time and not merely assumed downstream. The
three `call_result`, `const_name` and `other_machine` twins reject with the
stronger structural refusal ``the instantiated fact is structurally false
(`9 <= 5`)``; the other six reject with ``cannot prove requires contract for
call ...``. Both are compile-check refusals at the same obligation.

**Terminal Psi and carrier lowering.** All nine positives execute through the
shared Terminal core with `interp exit: 0`, and the corpus outcome rows
record `checked` for the nine `_exit` fixtures.

**Native realization and image emission.** All nine positives build and run
natively with `native exit 0` for the host target, and the native golden rows
record `built` with `exit: 0`.

Still open on `ONE-INTEGER-BOUNDS-ENGINE`: this row establishes the call-result
discharge of a bounded integer obligation only. It does not establish bounds
carried as borrowed projections, floating-point ranges, or generic
instantiations, and it does not close the parent capability.

## Measured evidence

Producer: `target/debug/omega`, rebuilt from this candidate, `omega
--build-profile` reports `opt_level 1`. Target: `macos_arm64`. The compiler
was rebuilt rather than reused because an incoming Psi change to call
evidence transport landed while this row was in flight, and a binary built
before it cannot establish behavior on the changed checker. Reusing the
older binary produced `compiler identity changed; run omega update`, which is
the documented signal that the lock belongs to a different producer.

Sample gate, `python3 tools/sample_native_gate.py --facts --filter
call_result_bound_discharge`:

```
sample_native_gate: 9/9 samples meet their documented oracle on macos_arm64
sample_native_gate: 9/9 intended-invalid controls match their authored rejection
```

Outcome leg, `python3 tools/corpus_gate.py --filter call_result_bound_discharge`:

```
corpus_gate: clean — 18 fixtures ({'checked': 9, 'rejected': 9}), 0 rejected
without their expected fragment, 0 fail fixtures did not reject
```

Native leg, `python3 tools/corpus_gate.py --native --filter
call_result_bound_discharge`:

```
corpus_gate: clean — 9 fixtures ({'built': 9})
executed 9 (exit codes {'0': 9}), 0 with stdout differing from expected
```

Both-engine leg, `python3 tools/corpus_gate.py --both --filter
call_result_bound_discharge`:

```
corpus_gate: clean — 9 fixtures ({'built': 9})
executed 9 (exit codes {'0': 9}), 0 with stdout differing from expected
```

That leg's golden rows record `exit:0 interpreter-exit:0 interpreter:agrees`,
which is the machine-readable form of native and Terminal agreement.

Each place also runs directly, and both engines agree:

```sh
omega run --both samples/cli/facts/call_result_bound_discharge/local/main.omg
# native exit 0
# interp exit: 0
```

## Oracle discrimination

The exit oracle is proved to discriminate rather than assumed. Corrupting the
observed result in the `local` place (`transition r == 4` to `transition r ==
5`) makes the same project trap under both engines:

```
native signal 5
interp crash: Trap (host presentation agrees)
```

Exit 255 against the authored 0. Likewise, setting one fixture's
`expected_native_exit.txt` to 7 makes the corpus gate report `expected native
7, observed built {'exit': '0'}` and count 1 unmet oracle. Both controls were
restored after measurement.

## Reject twins

Each twin is `<place>/controls/invalid/`, fed a closed result of 9. All nine
reject at `--check` with exit 1 and their authored fragment:

| Place | Authored diagnostic fragment |
| --- | --- |
| local | `cannot prove requires contract for call echo from Main::main: s <= 5` |
| field | `cannot prove requires contract for call echo from Main::main: reading.scale <= 5` |
| parameter | `cannot prove requires contract for call deliver from Main::main: s <= 5` |
| call result | ``the instantiated fact is structurally false (`9 <= 5`)`` |
| match join | `cannot prove requires contract for call echo from choose: scale <= 5` |
| const name | ``the instantiated fact is structurally false (`9 <= 5`)`` |
| another state | `cannot prove requires contract for call hold from carry: scale <= 5` |
| another machine | ``the instantiated fact is structurally false (`9 <= 5`)`` |
| another target body | `cannot prove requires contract for call take from Main::main: s <= 5` |

## Pinned fixtures

- Positives: `tests/omega/pass/facts/call_result_bound_discharge_through_<place>_exit`,
  each binding `Main::main` for the four hosted targets and carrying
  `expected_native_exit.txt` = 0.
- Twins: `tests/omega/fail/facts/call_result_bound_discharge_through_<place>_rejected`,
  each carrying its authored fragment in `expected.txt`.

Both sets are byte-identical to the corresponding sample place and twin, so
the fixture tree pins the same programs the row runs.
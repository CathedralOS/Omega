# Sample native oracles, macOS arm64

`tools/sample_native_gate.py` completes each sample's per-checkout package
review, builds it natively, runs it, and compares the sample's own documented
header (`Expected exit`, `Expected output contains`). This is the half of
SAMPLE-CORPUS that `tools/sample_gate.py` does not measure.

## 2026-09-27 at `4967132e292`

**83 of 143 samples meet their documented exit and output**, against
**119 of 148** that check. Five are unscored: four document no oracle, and
`samples/uefi/uefi_hello` is freestanding with no host oracle.

| state | count |
| --- | --- |
| pass (native build + documented exit + documented output) | 83 |
| no native build | 58 |
| no oracle documented | 4 |
| review/tooling error | 2 |
| not host-runnable | 1 |

**Read the check gate as necessary, not sufficient.** A rising check count is
not a rising native count: 36 samples check and do not reach a running
program.

## Why `omega refresh-samples` cannot produce this number

It never accepts trust on a sample's behalf, and samples carry no committed
lock. Run here it stopped at `package acceptance is missing` for **145 of
148** samples and never reached lowering — three had accepted reviews only
because this session had accepted them by hand. Completing the review is a
per-checkout step and the new gate performs it.

## The 58 that do not reach a running program

**27 of them never check**, so realization was never reachable; they are the
check gate's own refusals seen from here. With the two that exceeded the
per-call budget below, that accounts for all 29 the check gate reports.

The other **31 are genuine realization refusals: samples that check and
cannot be built.**

| count | refusal |
| --- | --- |
| 10 | `byte replacement requires bounded bytes and exact branch-free operands` — the [predicate byte carrier gap](predicate_byte_carrier_native_gap.md) |
| 9 | `InvalidUnitMachinePlan { machine: "darwin_poll" / "darwin_clock_ns", reason: "scalar callee has no checked executable body" }` |
| 3 | `OperationProofUnavailable` (`heat_grid`, `nested_counters`, `substring_search`) |
| 3 | `record operand lost its exact nominal source type` (`slice_maximum`, `clamp_sum`, `fletcher_checksum`) |
| 2 | `runtime field observation requires a record-only field path` (`brightness_control`, `shape_area`) |
| 1 | `UnsupportedByteSequenceLiteral` (`run_length_encode`) |
| 1 | `scalar graph slice element is not primitive` (`slice_accum_probe`) |
| 1 | `UnsupportedControlFlow` (`atomics_cross`) |
| 1 | `terminal-artifact production requires one exact selected program entry` (`trapping_probe`) |

### The whole corpus reconciles

| | count |
| --- | --- |
| check (`tools/sample_gate.py`) | 119 |
| — meet their documented oracle natively | **83** |
| — check but cannot be realized | **31** |
| — document no oracle (`wrapping_collatz_max`, `wrapping_square_sum`, `standalone`, `structural_proofs`) | 4 |
| — freestanding, no host oracle (`uefi_hello`) | 1 |
| do not check | 29 |
| total | 148 |

Two of the 29, `matrix_multiply` and `descriptor_walk`, appear here as
`review-timeout` rather than as refusals: they take 2232s and 805s to check,
which is how a long check presents to a gate with a per-call budget. See the
timings in [sample_gate_macos_arm64.md](sample_gate_macos_arm64.md). **A
timeout is not a refusal**; do not pool them.

### `darwin_poll` has no checked executable body

The nine are exactly the samples wired in the "Clock-bearing samples" row:
`binary_search_viz`, `sort_visualizer`, `bouncing_console`, `ripple_field`,
`tick_marquee`, `cellular_automaton`, `game_of_life_glider`, `langtons_ant`,
`random_walk`.

**Wiring the provider did not cause this and does not repair it.** Before the
wiring these samples were refused earlier, at provider selection, so they had
no native build either; they now check and still have none.

**It is not the provider indirection, and not one leaf.** Two controls, each a
ten-line program calling the leaf directly from `Main::main` with no provider
and no `satisfies` machine anywhere:

| program | result |
| --- | --- |
| `self.rc = darwin_poll(0, 0, 10);` | checks; `InvalidUnitMachinePlan { machine: "darwin_poll", reason: "scalar callee has no checked executable body" }` |
| `self.ns = darwin_clock_ns(6);` | checks; the same, naming `darwin_clock_ns` |

So every call to a `DarwinClock` leaf is unrealizable, however it is reached.
The leaves are not missing or unreachable: `macos_gui.omg:30` imports
`targets::macos_arm64::macos_gui_impl`, and that file binds each one — for
example `darwin_poll ... satisfies DarwinClock::poll via
darwin_poll_binding()`, a `MachODylibSymbol` for `_poll` in
`/usr/lib/libSystem.B.dylib`. A foreign-bound machine has no Omega body, and
the unit plan demands one for a scalar callee in this position.

Console leaves do realize — 83 samples print through them — so this is not
"foreign bindings do not lower" in general. What distinguishes the two was not
established here.

This qualifies the board's note that "existing macOS wiring is in
`samples/gui/window_app` and `source/library/std/macos_gui.omg`: exact Clock
selection and sleep through `darwin_poll`". That route reaches checked
semantics; it does not reach a running program, and `window_app` never
exposed it because `window_app` does not check at all — it is refused by the
[named operator handle mismatch](named_operator_selected_use.md).

So the Clock row's macOS leg is complete for checking and open for
realization. Closing it wants an executable body for the `DarwinClock` leaf
family, not more per-sample wiring; `source/library/std/clock.omg` declines
darwin for a different stated reason (the `invokes DarwinClock` ceiling leak)
and is right to for an additional one it does not mention.

## Reading this later

The gate accepts every pending review decision so the corpus can be measured
at all. That records project acceptance, **not** that an audit occurred, and
it writes only ignored files (`omega.lock`, `build/`). A pass row here means
the program built, ran, exited as documented and printed what it documents.
It does not mean its dependencies were audited.

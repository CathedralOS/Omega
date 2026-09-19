# D composition through the canonical evaluator edge

This explicit slow gate checks and executes Epsilon-written Omega compiler D
customers through the canonical section-11 evaluator edge — not the private
development transport. It reconstructs the canonical evaluator receipt
through the selected lower chain: the bound Gamma evaluator runs the bound
Delta compiler over the packed `epsilon_compiler.delta.sources` closure plus
the bound canonical entry
[`evaluator_entry.delta`](../evaluator-entry/evaluator_entry.delta). The
729,060-byte receipt (SHA-256
`bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368`) then
consumes each customer framed as one EREQ v1 envelope and publishes canonical
observations — `00` Exit, `01` Trap, `02` Reject — or an EEOUT refusal
frame / lower-chain `Incomplete` status, per
[`EVALUATOR_ENTRY.md`](../../../bootstrap/4_epsilon/EVALUATOR_ENTRY.md).

The development transport gate
[`tests/epsilon/interpreted-omega-experiment`](../interpreted-omega-experiment/README.md)
executes the same six whole-member customers through the private diagnostic
adapter. This gate carries the identical customers over the canonical
envelope: the Exit/Reject/Trap observation grammars are byte-identical by
design, so every expected observation below is the same byte string the
diagnostic gate pins. This covers the resource/entry-conformance leg of D
composition that the diagnostic transport does not reach.

Run from the repository root on macOS arm64, or Windows x64 in Git Bash:

```sh
sh tests/epsilon/d-composition/run.sh
```

Both routes require `python3`, Git's Unix tools on Windows, and the
corresponding checked-in Alpha seed; macOS also requires `codesign`. The
shell wrapper uses the shared bootstrap paths and bound materializers; Python
owns framing, invocation, and comparison on both hosts. Windows execution has
not yet been validated.

## Customers

The six whole-member customers concatenate the whole, unchanged D members
pinned by `omega_compiler.epsilon.sources` with the pinned customer entries
under `interpreted-omega-experiment/customers/`; member and packed identities
are re-pinned here as records of the same bound obligation. Their canonical
observations are the same exact byte strings the diagnostic gate expects:

| Customer | Packed bytes | Canonical observation |
| --- | ---: | --- |
| Omega D lexical helpers | 35,999 | `00 00000000 41` |
| Omega D Alpha tape buffers | 70,099 | `00 00000000 4142434445464748` + sealed 10-byte payload + sealed 79-byte program |
| Omega D request and UTF-8 | 65,181 | `00 00000000 410a` |
| Omega D request invocation fields | 61,601 | `00 00000000 410a` |
| Omega D numeric-base sums | 35,992 | `00 00000000 41` |
| Omega D lexer | 109,611 | `00 00000000 41` |
| Omega D complete closure | 511,026 | `00 00000000` + emitted tape ([`expected.hex`](expected.hex)) |
| Omega D complete closure check only | 509,512 | `00 00000000` |

On macOS arm64 at the recorded revision, the six whole-member customers each
produced their exact canonical observation in 61.336, 555.423, 668.186,
508.188, 137.523, and 453.776 seconds respectively (in table order), on a
host running concurrent bootstrap work; these are host measurements, not
bounds.

The seventh customer is the complete-D composition: the whole bound
509,267-byte packed D closure (`5c23b759…`) plus this gate's own
[`main.epsilon`](main.epsilon), which reads the sealed stdin section through
`Console.read_byte`, calls D's actual `OmegaScalarCompiler::compile` on the
[`program.omg`](program.omg) source (`machine answer() -> u8 { 42 }`,
selecting `answer`), and publishes the emitted Alpha tape bytes to stdout.
Checking therefore covers every declared D member in one closure, and
execution crosses `OmegaUtf8Validation`, the lexer, `OmegaParser::parse_view`,
scalar checking, `AlphaTapeBuffer` emission, and the outcome/status path.
The expected observation is `00` + exit 0 + the exact emitted tape, pinned in
[`expected.hex`](expected.hex). The stdin-driven composition main is the same
diagnostic harness shape the `tests/bootstrap/omega-executable` gate uses on
the development transport; the copy here is pinned so the canonical-edge
subject does not drift with that gate's edits.

The eighth customer is the check-only counterpart: the same whole closure
with [`check_only_main.epsilon`](check_only_main.epsilon), an entry that
falls off immediately with empty stdout (`00` + exit 0). Checking allocates
the same per the closure; its cumulative pair count isolates the checking
phase so the complete-closure run reads as checking plus execution. See the
allocation table below.

Selecting customers runs only the named members of the eight:

```sh
sh tests/epsilon/d-composition/run.sh 'Omega D lexer' 'Omega D complete closure'
```

The default observation watchdog is 14,400 seconds for each invocation;
receipt reconstruction has 900 seconds. These are host controls, not language
limits or successful resource-refusal judgments. For a slower host:

```sh
OMEGA_DCOMP_OBSERVATION_SECONDS=28800 sh tests/epsilon/d-composition/run.sh
```

## What this gate establishes

- The exact whole-D source closure — all eight manifested members — is
  checked and executed through the canonical evaluator edge: the selected
  lower chain plus the section-11 envelope, not the development adapter.
- The six whole-member customers produce identical canonical observations on
  the canonical edge, so the entry carries real D workloads without host
  parsing or policy.
- The complete-closure customer executes D's own `compile` machine: an
  ordinary Omega source compiles to an exact emitted Alpha tape published as
  canonical `Exit` stdout. This is composition evidence, not the final
  `omega0_compiler_bytecode.tape` product, which remains the OMEGA-D item's
  acceptance.

This gate does not establish the final section-11 acceptance: that still
requires independent `RunEpsilon` refinement agreement over the exact D
source (see `tests/epsilon/refinement/`), and resource conformance remains
bounded by the measured counters in `EVALUATOR_ENTRY.md`, not by these
execution times.

## Allocation measurement

The two whole-closure invocations at the recorded revision ran on macOS
arm64 under LLDB, which read the unchanged evaluator's Gamma heap cursor and
limit at `h_halt` (the two register words at `x19 + 0x500`, arena start
268,435,456, 40 bytes per pair, limit 1,879,048,192 bytes), then detached so
each process exited normally and published its canonical observation. Each
invocation consumed the identical request envelope the gate frames, and each
observation was byte-exact as pinned above.

| Observation | Check only | Complete |
| --- | ---: | ---: |
| Customer source bytes | 509,512 (packed D 509,267 + check-only main 245) | 511,026 (packed D 509,267 + main 1,759) |
| Wall time, canonical invocation | 3,305 s | 3,708 s |
| Final heap cursor | 422,360,496 | 494,819,216 |
| Cumulative allocation: cursor minus arena start | 153,925,040 (3,848,126 pairs) | 226,383,760 (5,659,594 pairs) |
| Remaining arena: limit minus cursor | 1,456,687,696 | 1,384,228,976 |

Receipt reconstruction through the bound Delta compiler measured 285.837 and
341.787 seconds in this gate's own invocations at the same revision. Both
evaluator invocations included an external suspension window of roughly
eleven minutes inside the wall times above; process CPU was about 24.5 and
30 minutes respectively on a host running concurrent bootstrap work.

Checking the whole 509,267-byte D closure consumes 9.56% of the pair arena;
the complete customer's execution adds 1,811,468 pairs for the scalar compile
of [`program.omg`](program.omg). The earlier whole-customer reading (80.05% in
the [parser comparison](../../bootstrap/omega-parser/README.md#field-identity-comparison))
was therefore execution-dominated, not a checking phase near the limit; at
the recorded revision no complete-D checking or single-compile execution
approaches the 1,879,048,192-byte boundary. These are host observations, not
profile bounds. An allocation result at or beyond the limit is an
owner-escalation finding under `bootstrap/MINIMIZATION.md`, not an
optimization task.

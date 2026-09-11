# Complete-D parser customer

This explicit slow gate checks and executes the complete manifested Epsilon
Omega compiler
closure plus one ordinary Epsilon `Main`. It reconstructs the existing Epsilon
execution diagnostic through the selected Gamma-authored Delta compiler, then
uses that receipt to check the customer and call D's real `OmegaParser::parse_view`.
No parser states are extracted, translated, or substituted by the host.

Run from the repository root on macOS arm64, or Windows x64 in Git Bash:

```sh
sh tests/bootstrap/omega-parser/run.sh
```

Both routes require `python3`, Git's Unix tools on Windows, and the corresponding
checked-in Alpha seed. macOS also requires `codesign`. The shell wrapper uses
the shared bootstrap paths and evaluator-stamping helpers; Python owns the
same framing, invocation, and comparison logic on both hosts. No PowerShell
installation is required. Windows execution has not yet been validated.

The customer reuses one parser across twelve invocations. Three complete
inputs cover decimal expressions and transition guards, a named struct literal,
a cast domain, reference/array/constrained types, and an externally bound
satisfying clause. Eight incomplete inputs exercise each relevant failing guard.
The no-clause external binding immediately follows a successful external leaf;
the initial data input repeats after failures to check reset behavior. Only
`Complete` permits syntax-row inspection; incomplete cases inspect status and
diagnostic spans, not partial syntax tables. [Fixture expectations](fixture.md)
derive the row counts and coordinates from the retained parser representation.

All assertions and status matches run in Epsilon. Success is the private
diagnostic `Exit(0)` with exact stdout `A`, recorded in `expected.hex`; the outer
Gamma process must also return zero with empty stderr. Other diagnostic tags
and raw evaluator failures fail the gate without being reclassified. The
diagnostic adapter and receipt retain their existing Epsilon-owned identities.
The host validates source manifests, concatenates whole sources, reconstructs
the receipt, prints the exact customer identity, and compares observations.

The default host observation allowance is 14,400 seconds for this complete-D
customer and 300 seconds for receipt reconstruction. It is a watchdog, not a
language limit or a successful resource-refusal judgment. For a slower host,
the same invocation can request more observation time:

```sh
OMEGA_PARSER_OBSERVATION_SECONDS=28800 sh tests/bootstrap/omega-parser/run.sh
```

This customer establishes only the tested parser behavior through the lower
chain. It does not establish the final Epsilon envelope, complete Omega grammar,
D semantic/lowering/emission closure, or compilation of C. Keep it separate from
the faster Epsilon fixture suite. Delete it only when a stronger full-D gate
subsumes these guard and repeated-invocation observations.

## Field-identity comparison

The baseline at `844e450838495884caf2676515d66ffc3ceace1c` and compact-field
candidate at `e60e1a8420f151236cc9d5d8055c8fd19fd2ecd6` both completed on macOS
arm64 on 2026-09-08 UTC using the command above. Both passed all twelve
invocation checks and returned `000000000041` (`Exit(0)`, stdout `A`), with
outer status zero and empty stderr. Alpha seed, Gamma evaluator source/tape,
customer, and resource provision were unchanged; folder moves changed no
executable bytes. These are parser-process observations, not whole-chain
rebuild or self-hosting times.

The 470,766-byte customer SHA-256 was
`c0b4bd072f6cd0d2b2ad16aebb327a027a86040d945ac1856a5ad62a9d96773f`.
The baseline execution receipt was 712,070 bytes, SHA-256
`c657884fde474a63c8ffafd628959c8c47e6a7440172d3a4d449991d1dab148f`.
The candidate receipt was 713,259 bytes, SHA-256
`de5ed0d307a218a6b99f0618f722eaeafa0d51a88b1aacadbdc51e3458a16c9a`.
Each revision's gate owns its exact source and adapter pins.

At each process's normal `h_halt`, LLDB read the unchanged arm64 evaluator's
Gamma heap cursor and limit from the two register words at `x19 + 0x500`, then
detached. The [selected pair arena](../../../bootstrap/2_gamma/EVALUATOR_PROFILE.md#private-partition)
starts at 268,435,456 and allocates 40 bytes per pair:

| Observation | Baseline | Compact fields |
| --- | ---: | ---: |
| Receipt reconstruction, seconds | 108.569 | 114.256 |
| Interpreted-parser process, seconds | 5,200.735 | 2,951.257 |
| Final heap cursor, bytes | 1,552,586,976 | 1,557,662,376 |
| Heap limit, bytes | 1,879,048,192 | 1,879,048,192 |
| Cumulative allocation: cursor minus arena start, bytes | 1,284,151,520 | 1,289,226,920 |
| Remaining arena: limit minus cursor, bytes | 326,461,216 | 321,385,816 |

The candidate took 49.188 minutes versus 86.679 minutes: 43.25% less wall time
in this pair. It allocated 126,885 additional pairs, or 5,075,400 bytes (0.40%
more), using 80.05% instead of 79.73% of the same 1,610,612,736-byte arena.
Allocation includes the entire interpreted-parser process, not the separate
receipt-reconstruction process; it is not peak live memory or RSS. Both runs
overlapped other bootstrap work; the baseline also included short OS sampling
and debugger pauses. This is one paired observation, not an isolated benchmark
series or a universal speedup.

Retain compact fields: this unchanged customer completes substantially sooner
for a small measured allocation increase without a new index, native engine,
profile, or proof rule. The representation adds three payload pairs per evaluated
projection construction, not once per populated field; net whole-run allocation
is measured above. Keeping syntax payloads and destructuring them directly
remains an unmeasured simpler-layout alternative, not a demonstrated improvement
over this result. The successful comparison closes this optimization's customer
check, not final Epsilon conformance, complete D, or any proof edge.

## Gamma scanning review

Retain the current Gamma scanner without an expression-end index. The complete
customer passes after compact field identities; the evidence below does not
demonstrate a need for more low-level state in this run-once chain. This is a
decision against additional machinery now, not a claim that scanning is cheap
or that a local optimization cannot help. The
[evaluator rationale](../../../bootstrap/2_gamma/EVALUATOR_PROFILE.md#expression-scanning)
compares the implementation and audit obligations.

On 2026-09-08 UTC at `9a658149720c5afbba5dc3011759f659cc752306`, a disposable
macOS arm64 probe reused the exact 713,259-byte compact-field receipt and
470,766-byte customer identified above, with the unchanged selected Gamma tape
and Alpha seed. It used the gate's framing and result comparison but did not
repeat receipt reconstruction. The process completed all twelve checks in
2,762.885 seconds (46.048 minutes), returning outer status zero, exact
`000000000041`, and empty stderr. Debugger pauses and OS sampling are included;
this is another successful execution, not a new optimization or speedup.

Three short LLDB windows read the Alpha program counter/return stack and Gamma
activation rows, with 120 snapshots per window at randomized 0.12–0.26-second
running intervals. A snapshot counted as skipping when its current Beta label
or an Alpha return-stack label belonged to `skip_expression`:

| Observed phase of the same complete customer | Skipping snapshots |
| --- | ---: |
| Epsilon lexical precheck of D source | 20 / 120 |
| Epsilon checking D expressions and continuations | 40 / 120 |
| Epsilon checking D names and transitions | 27 / 120 |

These are phase-local observations, not a whole-process cost distribution.
A later single snapshot reached D execution through Epsilon's runtime field
reader; the sustained runtime window was missed during an interruption and
supplies no runtime percentage. The older 37/160 D-runtime observation used
the pre-compact-field receipt and is not a current-runtime estimate.
Separately, macOS `sample <pid> 10` during the early source phase placed
6,712 of 7,807 top native PCs in Alpha's `next` dispatch. Native dispatch and
Gamma skipping overlap interpreter layers; their counts cannot be added or
read as a promised speedup. The probes were disposable, not repository tooling
or proof evidence. No Windows profiling result is claimed.

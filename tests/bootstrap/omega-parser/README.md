# Complete-D parser customer

This explicit slow gate checks and executes the complete six-member Epsilon D
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

The pre-change baseline at `844e450838495884caf2676515d66ffc3ceace1c` completed
on macOS arm64 on 2026-09-08 UTC using the command above. All twelve invocation
checks passed; the customer returned `000000000041` (`Exit(0)`, stdout `A`),
with outer status zero and empty stderr. Receipt reconstruction took 108.569
seconds; the separate interpreted-parser process took **5,200.735 seconds**
(86.679 minutes). This is not whole-chain rebuild or self-hosting time.

The 470,766-byte customer SHA-256 was
`c0b4bd072f6cd0d2b2ad16aebb327a027a86040d945ac1856a5ad62a9d96773f`.
The baseline execution receipt was 712,070 bytes, SHA-256
`c657884fde474a63c8ffafd628959c8c47e6a7440172d3a4d449991d1dab148f`.
The gate at that revision owns its exact source and adapter pins; the current
gate pins the compact-field candidate instead.

At the baseline's normal `h_halt`, LLDB read the unchanged arm64 evaluator's
Gamma heap cursor and limit from the two register words at `x19 + 0x500`, then
detached. The [selected pair arena](../../../bootstrap/2_gamma/EVALUATOR_PROFILE.md#private-partition)
starts at 268,435,456 and allocates 40 bytes per pair:

| Baseline allocation observation | Bytes |
| --- | ---: |
| Final heap cursor | 1,552,586,976 |
| Heap limit | 1,879,048,192 |
| Cumulative allocation: cursor minus arena start | 1,284,151,520 |
| Remaining arena: limit minus cursor | 326,461,216 |

That is 32,103,788 allocated pairs, about 79.73% of the 1,610,612,736-byte
arena. It includes the entire interpreted-parser process, not the separate
receipt-reconstruction process; it is not peak live memory or RSS. This single
run overlapped other bootstrap work and included short OS sampling/debugger
pauses, so its wall time is not an isolated benchmark.

The compact-field run at `e60e1a8420f151236cc9d5d8055c8fd19fd2ecd6` uses the
same customer and provision. Its full-parser time, outcome, and allocation
remain pending. The paired lexer result is not a substitute for that comparison;
no complete-parser speedup or allocation improvement is established yet.

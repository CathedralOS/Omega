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

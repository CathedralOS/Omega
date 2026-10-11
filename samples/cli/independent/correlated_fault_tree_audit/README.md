# Correlated fault tree audit

This complete Omega console application audits a declared synthetic instrument
model with three event IDs: A=0 (sensor), B=1 (power), C=2 (shared environment).
It is an illustrative finite probability model, not an assessment of hardware.

Input is thirteen unsigned decimal integers followed by EOF: active term count
(0..4), four term masks (0..7), then eight joint event weights in assignment-mask
order 0..7. A bit means that event is present. Each active term is an AND of its
events; the tree ORs the active terms. Inactive term slots must be zero. An active
zero term is unconditional failure; zero active terms means no failure.
Duplicates and subsumed terms are allowed. All eight weights must be 0..1000000
and sum to 1000000. The joint distribution may have arbitrary correlations and
zero-weight states. Whitespace separates tokens; negative numbers, junk,
missing/extra tokens, overlong tokens and inputs containing 256 bytes or more are rejected with exit 2 and `INVALID INPUT`.

The domain module enumerates all eight event assignments. It emits the failure
probability as an exact ppm fraction, then every structurally minimal cut mask
in increasing order. Cut sets remain structural even when they have zero
probability. Per event, the report emits marginal ppm, failure ppm after forcing
that event absent, reduction from baseline, and failure ppm after forcing it
present. These interventions retain the supplied weights of the other events;
they are not conditional probabilities and do not assume independence.

Parser arithmetic and accumulators explicitly use ordinary Wrapping semantics.
The parser admits at most seven digits and rejects values exceeding 1000000
before further digit accumulation. Thus its largest intermediate is 10000009;
the eight-weight total is at most 8000000. Accepted distributions bound every
risk accumulator by 1000000 and monotonicity makes reductions nonnegative.
No wrap is reachable. Array access and Exact cursors have local bounds guards.

The application owns its parser, term table, exhaustive Boolean evaluator,
minimal-cut reducer, intervention calculation and decimal reporter. It imports
only the existing core language facilities and standard Console provider.
`sensor_min_max` and `stats_compute` are numeric reductions, `format_number`
formats fixed values, and `module_file_chain` tests imports. None analyzes joint
Boolean event logic, minimal cut sets or interventions. This is a new workflow.

`data/frozen_oracle.json` freezes nine valid and twelve invalid runtime scenarios
before execution. `reference.py` independently enumerates Python sets and exact
integer sums to check the immutable output files; it is verification support,
not an application implementation. Numerical tolerance is zero. The frozen
manifest SHA-256 is
`5e6e90275952fe599793db1d04258d4c753ffcfe23daeb7105b99d3783c70918`.

From the repository root, using a compiler built at the recorded base:

```sh
export OMEGA_LIBRARY_ROOT="$PWD/source/library"
OMEGA=/absolute/path/to/verified/omega
PROJECT=samples/cli/independent/correlated_fault_tree_audit
"$OMEGA" update --project "$PROJECT" --target linux_x86_64
# Inspect any pending package review and accept only reviewed decisions.
# Resume once, after acceptance; a denied route remains a blocker.
"$OMEGA" update --resume --project "$PROJECT"
python3 "$PROJECT/reference.py"
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route check
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route native
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route terminal
python3 "$PROJECT/validate.py" --omega "$OMEGA" --route invalid
```

Run only one compiler execution at a time. `--both` uses finite stdin for native
and Terminal replay. The CLI compares termination, not interpreted stdout;
byte-exact native output checks and recorded Terminal agreement are distinct
evidence. Unsupported or declined interpretation is not a pass.

`controls/intended_invalid/` is a separate complete program with its own Build
entry binding. It deliberately writes element 3 of a three-element array.
The required rejection is `cannot prove index`, the documented collection bounds
rule; package/custody/review rejection is a wrong-reason failure. See
`wiki/spec/language/counts_and_addresses.md#bounds-and-lowering` for bounds obligations.
Generated route logs and native products stay under ignored `build/`.

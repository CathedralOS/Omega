# NF17 polynomial value and derivative

`scalar_cubic` evaluates binary64 constants and a cubic, including empty-input
try handling. `batch_quadratic` evaluates binary32 query backing slices into
caller-owned value and derivative backing slices, including output-size errors.
Each explicitly selects a named scalar implementation; no storage allocation,
coefficient reversal, format conversion, or descriptor implementation is hidden.
`invalid_empty` is a separate source project: its checked call with a zero-length
coefficient slice must reject the `coefficients.len > 0` precondition.
A package/import or parser failure is not the intended rejection.

Oracles in `oracles.json` were frozen before execution. Absolute and relative
tolerance are zero: all selected operations and answers are binary exact.
`verify_oracles.py` computes independent rational power sums and analytic slopes.
These fixtures do not impose an NF02 policy on general non-exact input.

From the repository root, run:

```sh
python3 samples/cli/scientific/w100/nf17_polynomial_horner_derivative/verify_oracles.py
python3 samples/cli/scientific/w100/nf17_polynomial_horner_derivative/prepare.py
omega --check samples/cli/scientific/w100/nf17_polynomial_horner_derivative/scalar_cubic/main.omg
omega run samples/cli/scientific/w100/nf17_polynomial_horner_derivative/scalar_cubic/main.omg
omega run --both samples/cli/scientific/w100/nf17_polynomial_horner_derivative/scalar_cubic/main.omg < /dev/null
omega --check samples/cli/scientific/w100/nf17_polynomial_horner_derivative/batch_quadratic/main.omg
omega run samples/cli/scientific/w100/nf17_polynomial_horner_derivative/batch_quadratic/main.omg
omega run --both samples/cli/scientific/w100/nf17_polynomial_horner_derivative/batch_quadratic/main.omg < /dev/null
omega --check samples/cli/scientific/w100/nf17_polynomial_horner_derivative/invalid_empty/main.omg
```

On Windows use `python` and an empty redirected input file for `--both`.
`prepare.py` explicitly copies the canonical kernel into each project's ignored
local module. This portable source materialization keeps NF01 package metadata
unmodified until its import interface is available. Re-run it after kernel edits.
Check, native execution, Terminal comparison, and intended rejection are separate
qualification outcomes. A declined Terminal comparison does not qualify it.

Current candidate status: mathematical fixtures verified; authored project routes
blocked by cache custody; direct kernel check still rejects source obligations.
See [the qualification report](verification/SUMMARY.md). This is not landing acceptance.

# Horner evaluation

```sh
omega run --both samples/cli/algorithms/horner_eval/main.omg
```

The program evaluates `x⁴ + 2x³ + 3x² + 4x + 5` at `x = 2`, checks the result
57, prints its success messages and exits 70 after Enter or EOF. Coefficients
stay in caller-owned storage borrowed by the nested machines implementing the
cyclic multiply-add computation. The step's bounds are an explicit entry
contract; the reporting helpers retain their blocking-call acknowledgements.

The public command needs the selected host's package review on a fresh checkout.
Inspect and decide that review before resuming `omega update`; the test harness
does not grant acceptance to this sample.

```sh
python3 tools/corpus_gate.py --both --filter collections/runtime_shared_array_polynomial_exit
```

This runs the byte-identical body under repository fixture admission. At
`e56af86474b` on macOS ARM64, native and checked-interpreter execution both exit
70 and match the three stdout lines. Public package acceptance remains a
separate requirement.
The public command at that revision exited 200 with pending package-review
decisions; no decisions were accepted for this measurement.

The nested-machine migration also executes the byte-identical body through
Terminal Psi and Windows x86-64 native execution: result 57, exit 70 and exact
stdout, including the terminal blocking calls to its reporting helpers.

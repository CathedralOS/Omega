# stats_compute

A statistics compute kernel that exercises f64 arithmetic end-to-end.

Computes mean, variance, and a weighted sum over a fixed 8-element dataset
(values: 2, 4, 4, 4, 5, 5, 7, 9).

- **Mean**: 5.0 (sum=40, count=8)
- **Variance**: 4.0 (sum of squared deviations = 32, divided by 8)
- **Weighted sum**: 82.5 (weights: 1.0, 1.0, 1.5, 1.5, 2.0, 2.0, 2.5, 3.0)

All checks pass via guard ladders and the program exits 70.

## What it tests

- f64 field arithmetic (`field + field`, `field * scalar`, `field - field`)
- f64 accumulation across multiple dispatched machine calls
- f64 comparison guards (`> 4.9`, `< 5.1`, etc.)
- Structs with `[copy]` property containing f64 fields
- Multiple sequential machine calls each updating shared float state

## Runtime acceptance

After project package review, run from the repository root with `omega` on
`PATH`:

```text
omega run samples/cli/arithmetic/stats_compute/main.omg
```

With closed stdin, the program prints these lines and exits 70:

```text
stats_compute: f64 mean, variance, and weighted sum over 8 samples
PASS: mean=5.0, variance=4.0, weighted sum=82.5 all in tolerance (exit 70)
[press Enter to close]
```

With interactive stdin, press Enter at the final prompt. Both the PASS line
and exit 70 are required; the introductory banner alone proves no result.

The development measurement route prepares local package acceptance, runs the
public command with closed stdin, and checks the authored output and exit:

```text
python3 tools/sample_native_gate.py --cli target/debug/omega --filter arithmetic/stats_compute --target macos_arm64 --jobs 1
```

This route records local acceptance for measurement, not a dependency audit.
Use Python 3 and `target/debug/omega.exe` on Windows, and select the matching
host target. macOS ARM64 execution is verified at `5094be283192`; Windows and
both Linux hosts still need matching-host runs. The unchanged program does not
reproduce the old non-tail-recursion blocker.
